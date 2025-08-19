use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

pub struct PlayerSeats {
    seats: Vec<PlayerSeat>,
}

#[derive(Clone)]
pub struct PlayerSeat {
    game_port: u16,
    in_use: bool,
}

impl PlayerSeat {
    /// Returns the game port for this seat.
    pub fn game_port(&self) -> u16 {
        self.game_port
    }
}

impl PlayerSeats {
    /// Open a player seat with the given port.
    pub fn openSeat(&mut self, port: u16) {
        if let Some(seat) = self.seats.iter_mut().find(|s| s.game_port == 0) {
            seat.game_port = port;
        }
    }

    /// Returns a player seat that is not currently in use.
    pub fn useSeat(&mut self) -> Option<PlayerSeat> {
        if let Some(seat) = self.seats.iter_mut().find(|s| !s.in_use) {
            seat.in_use = true;
            return Some(seat.clone());
        }
        None
    }

    /// Resets the player seats
    pub fn reset(&mut self) {
        self.seats = vec![
            PlayerSeat {
                game_port: 0,
                in_use: false,
            },
            PlayerSeat {
                game_port: 0,
                in_use: false,
            },
        ];
    }
}

pub static PLAYER_SEATS: Lazy<Arc<RwLock<PlayerSeats>>> = Lazy::new(|| {
    Arc::new(RwLock::new(PlayerSeats {
        seats: vec![
            PlayerSeat {
                game_port: 0,
                in_use: false,
            },
            PlayerSeat {
                game_port: 0,
                in_use: false,
            },
        ],
    }))
});
