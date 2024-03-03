use bytes::{Buf, Bytes, BytesMut};
use futures::prelude::*;
use futures::{
    task::{Context, Poll},
    Sink, Stream,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{collections::VecDeque, io, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use uuid::Uuid;

pub const WRITE_BUFFER_MAX_SIZE: u32 = 102400;

// Generic TCPDuplex struct
#[derive(Debug)]
pub struct TCPDuplex<T> {
    pub uuid: String,
    stream: TcpStream,
    write_buffer: VecDeque<Bytes>, // Outgoing messages queue
    read_buffer: BytesMut,         // Incoming data buffer
    _marker: std::marker::PhantomData<T>, // Marks generic type T
}

impl<T> TCPDuplex<T>
where
    T: Serialize + for<'de> Deserialize<'de> + std::marker::Unpin, 
{
    pub fn new(stream: TcpStream) -> Self {
        TCPDuplex {
            uuid: uuid::Uuid::new_v4().to_string(),
            stream,
            write_buffer: VecDeque::new(),
            read_buffer: BytesMut::with_capacity(4096),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn queue_message(&mut self, message: T) -> Result<(), serde_json::Error> {
        let serialized = serde_json::to_vec(&message)?;
        let length = serialized.len() as u32;
        let length_prefix = length.to_be_bytes();
        println!(
            "[{:}] Push to send #{:?} {:?}",
            self.uuid,
            Bytes::from(length_prefix.to_vec()),
            Bytes::from(serialized.clone()).len()
        );
        self.write_buffer.push_back(Bytes::from(length_prefix.to_vec()));
        self.write_buffer.push_back(Bytes::from(serialized));
        Ok(())
    }

    fn poll_flush_write_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let self_mut = self.get_mut();
        println!("[{:}] Flushed.", self_mut.uuid.to_string());
        while let Some(data) = self_mut.write_buffer.front() {
            let mut bytes_written = 0;
            while bytes_written < data.len() {
                match Pin::new(&mut self_mut.stream).poll_write(cx, &data[bytes_written..]) {
                    Poll::Ready(Ok(n)) if n == 0 => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "write zero byte",
                        )));
                    }
                    Poll::Ready(Ok(n)) => bytes_written += n,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            self_mut.write_buffer.pop_front(); // Message sent, remove from queue
        }

        Poll::Ready(Ok(()))
    }
}

impl<T> Sink<T> for TCPDuplex<T>
where
    T: Serialize + for<'de> Deserialize<'de> + std::marker::Unpin,
{
    type Error = io::Error;

    // fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
    //     println!("Start send msg");
    //     // Serialize the item and queue it for sending
    //     let serialized = serde_json::to_vec(&item).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    //     self.write_buffer.push_back(Bytes::from(serialized.clone()));
    //     println!("pushed {:?} SIZE:{:}", self.write_buffer,Bytes::from(serialized.clone()).len());
    //     if self.write_buffer.len() > WRITE_BUFFER_MAX_SIZE.try_into().unwrap() {
    //         println!("fk error");
    //         return Err(io::Error::new(io::ErrorKind::Other, "Write buffer full"));
    //     }
    //     Ok(())
    // }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        // Serialize and queue message
        println!("[{:}] Start send!", self.uuid.to_string());
        self.queue_message(item)?;

        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let self_mut = self.get_mut();
        println!("[{:}] Start flush!", self_mut.uuid.to_string());

        while let Some(data) = self_mut.write_buffer.pop_front() {
            match Pin::new(&mut self_mut.stream).poll_write(cx, &data) {
                Poll::Ready(Ok(_)) => (),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    self_mut.write_buffer.push_front(data);
                    return Poll::Pending;
                }
            }
        }

        // Flush stream
        match Pin::new(&mut self_mut.stream).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Backpressure management
        if self.write_buffer.len() < WRITE_BUFFER_MAX_SIZE.try_into().unwrap() {
            Poll::Ready(Ok(()))
        } else {
            // No actual backpressure management; always signals readiness. In practice,
            // consider using Poll::Pending and wake the task when buffer is below threshold.
            Poll::Ready(Ok(()))
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Close after all data is written
        match self.as_mut().poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> Stream for TCPDuplex<T>
where
    T: Serialize + for<'de> Deserialize<'de> + std::marker::Unpin + std::fmt::Debug,
{
    type Item = Result<T, io::Error>;


    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Handle incoming data
        println!("[{:}] Start read ", self.uuid.to_string());
        loop {
            // Length prefix check
            if self.read_buffer.len() >= 4 {
                let length = u32::from_be_bytes([
                    self.read_buffer[0],
                    self.read_buffer[1],
                    self.read_buffer[2],
                    self.read_buffer[3],
                ]) as usize;

                // Check if message is fully received
                if self.read_buffer.len() - 4 >= length {
                    self.read_buffer.advance(4); // Skip the length bytes
                    let data = self.read_buffer.split_to(length);

                    match serde_json::from_slice::<T>(&data) {
                        Ok(message) => return Poll::Ready(Some(Ok(message))),
                        Err(e) => {
                            return Poll::Ready(Some(Err(io::Error::new(
                                io::ErrorKind::Other,
                                e.to_string(),
                            ))))
                        }
                    }
                } else {
                    // Wait for more data
                    return Poll::Pending;
                }
            } else {
                // Insufficient data, read more
                let mut buf = [0; WRITE_BUFFER_MAX_SIZE as usize];
                let mut read_buf = ReadBuf::new(&mut buf);

                match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        if read_buf.filled().is_empty() {
                            // Stream end
                            return Poll::Ready(None);
                        } else {
                            // Append data and continue
                            self.read_buffer.extend_from_slice(read_buf.filled());
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
    }
}
