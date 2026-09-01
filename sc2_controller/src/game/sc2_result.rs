use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of a player
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sc2Result {
    Victory,
    Defeat,
    Tie,
    Crash,
    #[allow(clippy::upper_case_acronyms)]
    SC2Crash,
    Timeout,
    #[cfg(test)]
    Placeholder,
}
impl Sc2Result {
    pub fn from_proto(race: sc2_proto::sc2api::Result) -> Self {
        use sc2_proto::sc2api::Result;
        match race {
            Result::Victory => Self::Victory,
            Result::Defeat => Self::Defeat,
            Result::Tie => Self::Tie,
            Result::Undecided => panic!("Undecided result not allowed"),
        }
    }
}
impl fmt::Display for Sc2Result {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
