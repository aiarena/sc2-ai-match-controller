// StarCraft II specific race encoding. Will need to be generalized for other games in the future.

use serde::{Deserialize, Serialize};

#[derive(PartialOrd, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BotRace {
    NoRace = 0,
    Terran = 1,
    Zerg = 2,
    Protoss = 3,
    Random = 4,
}

impl BotRace {
    pub fn from_str(race: &str) -> Self {
        match &race.to_lowercase()[..] {
            "p" | "protoss" | "race.protoss" | "3" => Self::Protoss,
            "t" | "terran" | "race.terran" | "1" => Self::Terran,
            "r" | "random" | "race.random" | "4" => Self::Random,
            "z" | "zerg" | "race.zerg" | "2" => Self::Zerg,
            _ => Self::NoRace,
        }
    }
}
