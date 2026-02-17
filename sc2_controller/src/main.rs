mod game;
mod logging;
mod player_seats;
mod routes;
mod websocket;
mod ws_routes;

use crate::logging::init_logs;
use crate::routes::open_player_seat;
use tracing::info;

#[tokio::main]
async fn main() {
    let _guards = init_logs();

    let seat1 = open_player_seat(1).await;
    let seat2 = open_player_seat(2).await;

    match (seat1, seat2) {
        (Ok(ws1), Ok(ws2)) => {
            info!("Player seats opened successfully.");

            tokio::select! {
                _ = ws1 => info!("Player seat 1 exited."),
                _ = ws2 => info!("Player seat 2 exited."),
            }
        }
        (Err(e), _) | (_, Err(e)) => {
            panic!("Failed to start SC2: {:?}", e);
        }
    }
}
