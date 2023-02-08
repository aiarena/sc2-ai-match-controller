pub use utilities::portpicker;

pub mod api;
pub mod configuration;
pub mod logging;
pub mod models;
pub mod paths;
pub mod procs;
pub mod utilities;

#[cfg(feature = "swagger")]
use utoipa::ToSchema;

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlayerNum {
    One,
    Two,
}

impl PlayerNum {
    pub fn other_player(&self) -> PlayerNum {
        match self {
            PlayerNum::One => PlayerNum::Two,
            PlayerNum::Two => PlayerNum::One,
        }
    }
}
