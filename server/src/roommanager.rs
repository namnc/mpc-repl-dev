use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use warp::ws::{Message, WebSocket};

type UserId = String;
type RoomId = String;
type Room = HashMap<UserId, UserSender>;
pub type Rooms = Arc<Mutex<HashMap<RoomId, Room>>>;
type UserSender = Arc<Mutex<SplitSink<WebSocket, Message>>>;

#[derive(Debug, Deserialize, Serialize)]
struct RoomAction {
    action: String,
    room_id: RoomId,
    user_id: UserId,
    data: Option<String>,
}

#[derive(Debug, Serialize)]
struct OutgoingMessage {
    action: String,
    content: Option<String>,
    users: Option<Vec<UserId>>,
}

pub async fn handle_websocket(ws: WebSocket, user_id: UserId, room_id: RoomId, rooms: Rooms) {
    let (user_ws_tx, mut user_ws_rx) = ws.split();

    // Add the user to the room
    {
        let mut rooms = rooms.lock().unwrap();
        let room = rooms.entry(room_id.clone()).or_default();
        let user_sender = Arc::new(Mutex::new(user_ws_tx));
        room.insert(user_id.clone(), user_sender);
    }

    while let Some(result) = user_ws_rx.next().await {
        match result {
            Ok(msg) => {
                let msg_text = msg.to_str().unwrap_or_default();
                if let Ok(action_message) = serde_json::from_str::<Value>(&msg_text) {
                    if let Some(action) = action_message.get("action").and_then(Value::as_str) {
                        match action {
                            "listUsers" => {
                                let user_list = list_users_in_room(&room_id, &rooms).await;
                                let response = OutgoingMessage {
                                    action: "userList".to_string(),
                                    content: None,
                                    users: Some(user_list),
                                };
                                let response_text = serde_json::to_string(&response).unwrap();

                                send_message(&user_id, &room_id, &rooms, response_text).await;
                            }
                            _ => eprintln!("Unsupported action: {}", action),
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Remove the user from the room
    {
        let mut rooms = rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(&room_id) {
            room.remove(&user_id);
        }
    }
}

async fn list_users_in_room(room_id: &RoomId, rooms: &Rooms) -> Vec<UserId> {
    let rooms = rooms.lock().unwrap();
    rooms
        .get(room_id)
        .map_or_else(Vec::new, |room| room.keys().cloned().collect())
}

async fn send_message(user_id: &UserId, room_id: &RoomId, rooms: &Rooms, message: String) {
    let rooms_guard = rooms.lock().unwrap();
    if let Some(room) = rooms_guard.get(room_id) {
        if let Some(user_sender) = room.get(user_id) {
            let mut sender_guard = user_sender.lock().unwrap();
            let sending = async {
                if let Err(e) = sender_guard.send(Message::text(message)).await {
                    eprintln!("Error sending message to user {}: {}", user_id, e);
                }
            };
            sending.await;
        }
    }
}
