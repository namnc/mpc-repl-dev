use futures::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

pub type UserId = String;
pub type RoomId = String;
pub type Room = HashMap<UserId, User>;
pub type Rooms = Arc<Mutex<HashMap<RoomId, Room>>>;
pub type UserSender = mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>;

#[derive(Debug, Deserialize, Serialize)]
enum Action {
    CreateRoom,
    JoinRoom,
    LeaveRoom,
    ListUsers,
    SendDirectMessage,
    BroadcastMessage,
    Error,
    // Add more actions as needed
}

#[derive(Debug, Serialize, Deserialize)]
enum RoomError {
    RoomAlreadyExists(String),
    RoomDoesNotExists(String),
    TargetUserNotFound(String, String),
    MissingData(String, String, Action),
    LockFailed,
    // Add more errors as needed
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RoomError::RoomAlreadyExists(ref room_id) => {
                write!(f, "Room {} already exists", room_id)
            }
            RoomError::RoomDoesNotExists(ref room_id) => {
                write!(f, "Room {} does not exist", room_id)
            }
            RoomError::TargetUserNotFound(ref room_id, ref user_id) => {
                write!(f, "Target user {} in room {} does not exist", user_id, room_id)
            }
            RoomError::MissingData(ref room_id, ref user_id ,ref action )=>{
                write!(f, "Action {:?} from user {} in room {} is missing", action, user_id, room_id)
            }
            RoomError::LockFailed => write!(f, "Failed to acquire lock"),
        }
    }
}

impl std::error::Error for RoomError {}

#[derive(Debug, Deserialize, Serialize)]
struct RoomAction {
    action: Action,
    room_id: RoomId,
    user_id: UserId,
    data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
enum RoomResponse {
    Success(RoomAction),
    Error(RoomError),
}

#[derive(Serialize, Deserialize)]
struct DirectMessage {
    sender_id: String,
    receiver_id: String,
    content: String,
}

pub type PublicKey = String;
#[derive(Debug, Clone)]
pub struct User {
    user_id: UserId,
    public_key: PublicKey,
    sender: UserSender,
}

impl User {
    pub fn new(user_id: &UserId, user_pk: PublicKey, user_sender: UserSender) -> Self {
        Self {
            user_id: user_id.to_string(),
            public_key: user_pk,
            sender: user_sender,
        }
    }
    pub fn id(&self) -> &UserId {
        return &self.user_id;
    }
    pub fn publickey(&self) -> &PublicKey {
        return &self.public_key;
    }
    pub fn sender(&self) -> &UserSender {
        return &self.sender;
    }
}

async fn create_room(user: &User, rooms: &Rooms) -> Result<RoomAction, RoomError> {
    let mut rooms = rooms.lock().map_err(|_| RoomError::LockFailed)?;
    // Check if the room already exists
    let mut room_uuid = Uuid::new_v4();
    loop {
        if !rooms.contains_key(&room_uuid.to_string()) {
            break;
        }
        room_uuid = Uuid::new_v4();
    }
    let room_id = room_uuid.to_string();
    // // Proceed to create the room and add the user
    let room = rooms.entry(room_id.clone()).or_default();
    room.insert(user.id().clone(), user.clone());
    println!("Room {} created by user {}", room_id, user.id());
    Ok(RoomAction {
        action: Action::CreateRoom,
        room_id: room_id.clone(),
        user_id: user.id().to_string(),
        data: Some(format!("Room {} created", room_id)),
    })
}

async fn join_room(
    room_id: &RoomId,
    user: &User,
    rooms: &mut Rooms,
) -> Result<RoomAction, RoomError> {
    let mut rooms = rooms.lock().map_err(|_| RoomError::LockFailed)?;
    // Check if the room exists
    if let Some(room) = rooms.get_mut(room_id) {
        room.insert(user.id().to_string(), user.clone());

        let user_list = room.keys().cloned().collect::<Vec<_>>();

        // Broadcast the list of users to all users in the room
        for (user_id, user_in_room) in room.iter() {
            if user_id != user.id() {
                let message = RoomAction {
                    action: Action::ListUsers,
                    room_id: room_id.clone(),
                    user_id: user_in_room.user_id.clone(),
                    data: Some(serde_json::to_string(&user_list).unwrap()),
                };

                let json_message = serde_json::to_string(&message).unwrap();
                if let Err(e) = user_in_room.sender.send(Ok(Message::text(json_message))) {
                    eprintln!("Error sending user list: {}", e);
                }
            }
        }

        println!("User {} joined room {}", user.id(), room_id);
        Ok(RoomAction {
            action: Action::JoinRoom,
            room_id: room_id.clone(),
            user_id: user.id().to_string(),
            data: Some(format!("User {} joined room {}", user.id(), room_id)),
        })
    } else {
        return Err(RoomError::RoomDoesNotExists(room_id.clone()));
    }
}

async fn leave_room(
    room_id: &RoomId,
    user: &User,
    rooms: &mut Rooms,
) -> Result<RoomAction, RoomError> {
    let mut rooms = rooms.lock().unwrap();
    // Check if the room exists
    if let Some(room) = rooms.get_mut(room_id) {
        room.remove(user.id());
        println!("User {} left room {}", user.id(), room_id);
        if room.is_empty() {
            rooms.remove(room_id);
            println!("Removed room {}", room_id);
        }
        Ok(RoomAction {
            action: Action::LeaveRoom,
            room_id: room_id.clone(),
            user_id: user.id().to_string(),
            data: Some(format!("User {} leave room {}", user.id(), room_id)),
        })
    } else {
        return Err(RoomError::RoomDoesNotExists(room_id.clone()));
    }
}

async fn get_list_users_in_room(
    room_id: &RoomId,
    user: &User,
    rooms: &mut Rooms,
) -> Result<RoomAction, RoomError> {
    let mut rooms = rooms.lock().map_err(|_| RoomError::LockFailed)?;
    // Check if the room exists
    if let Some(room) = rooms.get_mut(room_id) {
        let user_list = room.keys().cloned().collect::<Vec<_>>();
        Ok(RoomAction {
            action: Action::ListUsers,
            room_id: room_id.clone(),
            user_id: user.id().to_string(),
            data: Some(serde_json::to_string(&user_list).unwrap()),
        })
    } else {
        return Err(RoomError::RoomDoesNotExists(room_id.clone()));
    }
}

pub async fn handle_connection(ws: WebSocket, user_id: UserId, mut rooms: Rooms) {
    let (user_ws_tx, mut user_ws_rx) = ws.split();
    let (user_sender, user_rcv) = mpsc::unbounded_channel();

    let client_rcv = UnboundedReceiverStream::new(user_rcv);
    tokio::task::spawn(client_rcv.forward(user_ws_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    let user = User::new(&user_id, format!("userid<{}>", user_id), user_sender);
    let mut heartbeat_interval = interval(Duration::from_secs(30)); // 30 seconds
    loop {
        tokio::select! {
            Some(result) = user_ws_rx.next() => {
                match result {
                    Ok(msg) => {
                        let json_response : String;
                        let response: RoomResponse;
                        let msg_text = msg.to_str().unwrap_or_default();
                        if let Ok(cmd_msg) = serde_json::from_str::<RoomAction>(msg_text) {
                            match cmd_msg.action {
                                Action::CreateRoom => {
                                    response = match create_room(&user, &mut rooms).await {
                                        Ok(action) => RoomResponse::Success(action),
                                        Err(error) => RoomResponse::Error(error),
                                    };
                                }
                                Action::JoinRoom => {
                                    response = match join_room(&cmd_msg.room_id, &user, &mut rooms).await {
                                        Ok(action) => RoomResponse::Success(action),
                                        Err(error) => RoomResponse::Error(error),
                                    };
                                }
                                Action::LeaveRoom => {
                                    response = match leave_room(&cmd_msg.room_id, &user, &mut rooms).await {
                                        Ok(action) => RoomResponse::Success(action),
                                        Err(error) => RoomResponse::Error(error),
                                    };
                                }
                                Action::ListUsers => {
                                    response = match get_list_users_in_room(&cmd_msg.room_id, &user, &mut rooms).await {
                                        Ok(action) => RoomResponse::Success(action),
                                        Err(error) => RoomResponse::Error(error),
                                    };
                                }
                                Action::SendDirectMessage => {
                                    // `data` field contains the target user ID and the message
                                    if let Some(data) = cmd_msg.data {
                                        let (target_user_id, message) = parse_direct_message_data(&data);
                                        response = match send_direct_message(&cmd_msg.room_id, &user, &rooms, &target_user_id, &message).await {
                                            Ok(_) => RoomResponse::Success(RoomAction{ action: todo!(), room_id: todo!(), user_id, data: todo!() }),
                                            Err(error) => RoomResponse::Error(error),
                                        };
                                    } else {
                                        response = RoomResponse::Error(RoomError::MissingData(cmd_msg.room_id.to_string(),cmd_msg.user_id.to_string(), cmd_msg.action));
                                    }
                                },
                                Action::BroadcastMessage => {
                                    if let Some(data) = cmd_msg.data {
                                        let (_, message) = parse_direct_message_data(&data);
                                        response = match broadcast_message_to_room(&cmd_msg.room_id, &user, &rooms, &message).await {
                                            Ok(_) => RoomResponse::Success(RoomAction{ action: todo!(), room_id: todo!(), user_id, data: todo!() }),
                                            Err(error) => RoomResponse::Error(error),
                                        };
                                    } else {
                                        response = RoomResponse::Error(RoomError::MissingData(cmd_msg.room_id.to_string(),cmd_msg.user_id.to_string(), cmd_msg.action));
                                    }
                                },
                                //Action::NewAction => { handle action }

                                _ => {
                                    eprintln!("Unsupported action: {:?}", cmd_msg.action);
                                    continue;
                                },
                            }
                            json_response = serde_json::to_string(&response).expect("Failed to serialize response");
                            // Send the response back to the client
                            if let Err(e) = user.sender.send(Ok(Message::text(json_response))) {
                                eprintln!("Error sending response to websocket: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            _ = heartbeat_interval.tick() => {
                if let Err(e) = user.sender.send(Ok(Message::ping(Vec::new()))) {
                    eprintln!("Error sending ping: {}", e);
                    break;
                }
            }
        }
    }
}

fn parse_direct_message_data(data: &str) -> (UserId, String) {
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (String::new(), String::new())
    }
}

async fn send_direct_message(
    room_id: &RoomId,
    user: &User,
    rooms: &Rooms,
    target_user_id: &UserId,
    message: &str,
) -> Result<(), RoomError> {
    let rooms = rooms.lock().unwrap();
    if let Some(room) = rooms.get(room_id) {
        if let Some(target_user) = room.get(target_user_id) {
            let msg = DirectMessage {
                sender_id: user.id().to_string(),
                receiver_id: target_user_id.to_string(),
                content: message.to_string(),
            };
            let smsg = serde_json::to_string(&msg);
            match smsg {
                Ok(json) => {
                    println!("Serialized message: {}", json);
                    let _ = target_user.sender().send(Ok(Message::text(json)));
                },
                Err(e) => eprintln!("Failed to serialize message: {}", e),
            }
            
        }
    }
    Err(RoomError::TargetUserNotFound(room_id.clone(),target_user_id.clone()))
}

async fn broadcast_message_to_room(
    room_id: &RoomId,
    user: &User,
    rooms: &Rooms,
    message: &str,
) -> Result<(), RoomError> {
    let rooms = rooms.lock().unwrap();
    if let Some(room) = rooms.get(room_id) {
        for (_, target_user) in room {
            let msg = DirectMessage {
                sender_id: user.id().to_string(),
                receiver_id: target_user.id().to_string(),
                content: message.to_string(),
            };
            let smsg = serde_json::to_string(&msg);
            match smsg {
                Ok(json) => {
                    println!("Serialized message: {}", json);
                    let _ = target_user.sender().send(Ok(Message::text(json)));
                },
                Err(e) => eprintln!("Failed to serialize message: {}", e),
            }
        }
        return Ok(());
    }
    Err(RoomError::LockFailed)
}
