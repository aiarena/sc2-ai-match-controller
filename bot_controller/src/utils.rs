use std::io;

use common::{configuration::ac_config::ACConfig, PlayerNum};

pub(crate) fn move_bot_to_internal_dir(
    settings: &ACConfig,
    bot_path: &str,
    player_num: PlayerNum,
) -> io::Result<String> {
    match player_num {
        PlayerNum::One => {
            std::fs::copy(bot_path, &settings.bot1_directory)?;
            Ok(settings.bot1_directory.clone())
        }
        PlayerNum::Two => {
            std::fs::copy(bot_path, &settings.bot2_directory)?;
            Ok(settings.bot2_directory.clone())
        }
    }
}
