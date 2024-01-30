
mod roommanager;
mod ws;
use roommanager::{handle_connection, Rooms};
use warp::Filter;

#[tokio::main]
async fn main() {
    let rooms: Rooms = Default::default();
    let health_route = warp::path!("health").map(|| format!("Server OK"));

    let websocket_route = warp::path("ws")
        .and(warp::path::param::<String>()) // User ID
        .and(warp::ws())
        .and(warp::any().map(move || rooms.clone()))
        .map(|user_id: String, ws: warp::ws::Ws, rooms: Rooms| {
            ws.on_upgrade(move |socket|{
                handle_connection(socket, user_id.clone(), rooms.clone())
        })
        });
    let routes = health_route
        .or(websocket_route)
        // .or(publish)
        .with(warp::cors().allow_any_origin());
    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;
}
fn with_clients(rooms: Rooms) -> impl Filter<Extract = (Rooms,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rooms.clone())
  }