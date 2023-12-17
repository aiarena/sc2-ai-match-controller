use crate::utilities::portpicker::Port;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ACConfig {
    #[serde(skip_serializing)] // Don't expose via config endpoint
    pub api_token: Option<String>,
    pub arena_client_id: String,
    pub base_website_url: String,
    pub bot1_directory: String,
    pub bot2_directory: String,
    pub bots_directory: String,
    pub bot_cont_1_host: String,
    pub bot_cont_1_port: Port,
    pub bot_cont_2_host: String,
    pub bot_cont_2_port: Port,
    pub debug_mode: bool,
    pub disable_debug: bool,
    pub hash_check: bool,
    pub logging_level: String,
    pub log_root: String,
    pub matches_file: String,
    pub max_frame_time: i32,
    pub max_game_time: u32,
    pub max_real_time: i64,
    pub timeout_secs: u64,
    pub python: String,
    pub realtime: bool,
    pub replays_directory: String,
    pub results_file: String,
    pub rounds_per_run: i64,
    pub run_type: RunType,
    pub sc2_cont_host: String,
    pub sc2_cont_port: Port,
    pub secure_mode: bool,
    pub strikes: i32,
    pub temp_path: String,
    pub temp_root: String,
    pub validate_race: bool,
    pub visualize: bool,
    pub aws: bool,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum RunType {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "aiarena")]
    AiArena,
    #[serde(rename = "test")]
    Test,
    #[serde(rename = "mock")]
    Mock,
}

impl Default for RunType {
    fn default() -> Self {
        Self::Local
    }
}

#[cfg(test)]
mod tests {
    use crate::configuration::ac_config::{ACConfig, RunType};

    fn ac_config() -> ACConfig {
        ACConfig {
            api_token: Some("123".to_string()),
            arena_client_id: "123".to_string(),
            base_website_url: "123".to_string(),
            bot1_directory: "123".to_string(),
            bot2_directory: "123".to_string(),
            bots_directory: "123".to_string(),
            bot_cont_1_host: "123".to_string(),
            bot_cont_1_port: 0,
            bot_cont_2_host: "123".to_string(),
            bot_cont_2_port: 0,
            debug_mode: false,
            disable_debug: false,
            hash_check: false,
            logging_level: "123".to_string(),
            log_root: "123".to_string(),
            matches_file: "123".to_string(),
            max_frame_time: 0,
            max_game_time: 0,
            max_real_time: 0,
            timeout_secs: 0,
            python: "123".to_string(),
            realtime: false,
            replays_directory: "123".to_string(),
            results_file: "123".to_string(),
            rounds_per_run: 0,
            run_type: RunType::Local,
            sc2_cont_host: "123".to_string(),
            sc2_cont_port: 0,
            secure_mode: false,
            strikes: 0,
            temp_path: "123".to_string(),
            temp_root: "123".to_string(),
            validate_race: false,
            visualize: false,
            aws: false,
        }
    }

    #[test]
    fn test_api_key_obfuscation() {
        let ac_config = ac_config();
        let serialized = serde_json::to_string(&ac_config).expect("Could not serialize ac_config");

        assert_eq!(ac_config.api_token, Some("123".to_string()));

        let deserialized: ACConfig =
            serde_json::from_str(&serialized).expect("Could not deserialize ac_config");
        assert!(deserialized.api_token.is_none());
    }
}
