
mod roommanager;
use roommanager::{handle_websocket, Rooms};
use warp::Filter;

#[tokio::main]
async fn main() {
    let rooms: Rooms = Default::default();

    let websocket_route = warp::path("ws")
        .and(warp::path::param::<String>()) // Room ID
        .and(warp::path::param::<String>()) // User ID
        .and(warp::ws())
        .and(warp::any().map(move || rooms.clone()))
        .map(|room_id: String, user_id: String, ws: warp::ws::Ws, rooms: Rooms| {
            ws.on_upgrade(move |socket|{
                 handle_websocket(socket, user_id.clone(), room_id.clone(), rooms.clone())
        })
        });

    warp::serve(websocket_route)
        .run(([127, 0, 0, 1], 8081))
        .await;
}
