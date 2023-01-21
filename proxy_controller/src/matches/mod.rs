use crate::game::race::BotRace;
use crate::matches::sources::file_source::errors::FileMatchExtractError;
use common::api::api_reference::aiarena::AiArenaMatch;
use common::models::bot_controller::{BotType, PlayerNum};
use std::collections::HashMap;
use std::str::FromStr;

pub mod aiarena_result;
pub mod sources;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchPlayer {
    pub id: String,
    pub name: String,
    pub race: BotRace,
    pub bot_type: BotType,
}

impl MatchPlayer {
    pub fn from_aiarena_match(player_num: PlayerNum, ai_match: &AiArenaMatch) -> Self {
        match player_num {
            PlayerNum::One => Self {
                id: ai_match.bot1.game_display_id.clone(),
                name: ai_match.bot1.name.clone(),
                race: BotRace::from_str(&ai_match.bot1.plays_race),
                bot_type: BotType::from_str(&ai_match.bot1._type).unwrap(),
            },
            PlayerNum::Two => Self {
                id: ai_match.bot2.game_display_id.clone(),
                name: ai_match.bot2.name.clone(),
                race: BotRace::from_str(&ai_match.bot2.plays_race),
                bot_type: BotType::from_str(&ai_match.bot2._type).unwrap(),
            },
        }
    }

    pub fn from_file_source(bot_line: &[String]) -> Result<Self, FileMatchExtractError> {
        Ok(Self {
            id: bot_line
                .get(0)
                .ok_or_else(|| FileMatchExtractError::PlayerId(bot_line.to_vec()))?
                .to_string(),
            name: bot_line
                .get(1)
                .ok_or_else(|| FileMatchExtractError::PlayerName(bot_line.to_vec()))?
                .to_string(),
            race: BotRace::from_str(
                bot_line
                    .get(2)
                    .ok_or_else(|| FileMatchExtractError::PlayerRace(bot_line.to_vec()))?,
            ),
            bot_type: BotType::from_str(
                bot_line
                    .get(3)
                    .ok_or_else(|| FileMatchExtractError::PlayerType(bot_line.to_vec()))?,
            )
            .map_err(|_| FileMatchExtractError::PlayerType(bot_line.to_vec()))?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Match {
    pub match_id: u32,
    pub players: HashMap<PlayerNum, MatchPlayer>,
    pub map_name: String,
    pub aiarena_match: Option<AiArenaMatch>,
}

impl From<AiArenaMatch> for Match {
    fn from(ai_match: AiArenaMatch) -> Self {
        let mut players = HashMap::with_capacity(2);
        players.insert(
            PlayerNum::One,
            MatchPlayer::from_aiarena_match(PlayerNum::One, &ai_match),
        );

        players.insert(
            PlayerNum::Two,
            MatchPlayer::from_aiarena_match(PlayerNum::Two, &ai_match),
        );

        Self {
            match_id: ai_match.id,
            players,
            map_name: ai_match.map.name.clone(),
            aiarena_match: Some(ai_match),
        }
    }
}
