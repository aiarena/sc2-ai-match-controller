use common::configuration::ac_config::ACConfig;

#[derive(Clone)]
pub struct PlayerSeat {
    pub settings: ACConfig,

    pub player_num: u8,

    // The port exposed to players
    pub external_port: u16,

    // The port to the SC2 process
    pub internal_port: u16,
}

impl PlayerSeat {
    pub fn new(settings: ACConfig, num: u8, port: u16) -> Self {
        PlayerSeat {
            settings,
            player_num: num,
            external_port: get_external_port(num),
            internal_port: port,
        }
    }
}

fn get_external_port(num: u8) -> u16 {
    let env_var = format!("ACSC2_PLAYER_{}_SEAT", num);
    let value = std::env::var(&env_var).unwrap_or_else(|_| {
        panic!("Missing {} environment variable", env_var);
    });
    value.parse().unwrap_or_else(|_| {
        panic!("Invalid {} environment variable", env_var);
    })
}
