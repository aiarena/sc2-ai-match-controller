use core::fmt;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
pub enum AiArenaResult {
    Player1Crash,
    Player2Crash,
    Player1TimeOut,
    Player2TimeOut,
    Player1Win,
    Player2Win,
    Tie,
    InitializationError,
    Error,
    Placeholder,
}

impl Display for AiArenaResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl FromStr for AiArenaResult {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Player1Crash" => Ok(Self::Player1Crash),
            "Player2Crash" => Ok(Self::Player2Crash),
            "Player1TimeOut" => Ok(Self::Player1TimeOut),
            "Player2TimeOut" => Ok(Self::Player2TimeOut),
            "Player1Win" => Ok(Self::Player1Win),
            "Player2Win" => Ok(Self::Player2Win),
            "Tie" => Ok(Self::Tie),
            "InitializationError" => Ok(Self::InitializationError),
            "Error" => Ok(Self::Error),
            #[cfg(test)]
            "Placeholder" => Ok(Self::Placeholder),
            _ => Err(()),
        }
    }
}
