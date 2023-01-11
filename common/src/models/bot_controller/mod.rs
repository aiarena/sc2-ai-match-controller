use crate::portpicker::Port;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StartBot {
    pub bot_name: String,
    pub bot_type: BotType,
    pub opponent_id: String,
    pub player_num: PlayerNum,
    pub match_id: u32,
    pub process_key: Port,
    pub should_download: bool,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum BotType {
    #[serde(rename = "cppwin32")]
    CppWin32,
    #[serde(rename = "cpplinux")]
    CppLinux,
    #[serde(rename = "dotnetcore")]
    DotnetCore,
    #[serde(rename = "java")]
    Java,
    #[serde(rename = "nodejs")]
    NodeJs,
    #[serde(rename = "python")]
    Python,
}

impl FromStr for BotType {
    type Err = ();
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        match &t.to_lowercase()[..] {
            "cppwin32" => Ok(Self::CppWin32),
            "cpplinux" => Ok(Self::CppLinux),
            "dotnetcore" => Ok(Self::DotnetCore),
            "java" => Ok(Self::Java),
            "nodejs" => Ok(Self::NodeJs),
            "python" => Ok(Self::Python),
            _ => Err(()),
        }
    }
}

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

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone)]
pub struct MapData {
    pub query: String,
    pub map_path: String,
}
