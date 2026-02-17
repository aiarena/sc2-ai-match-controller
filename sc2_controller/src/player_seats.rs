#[derive(Clone)]
pub struct PlayerSeat {
    pub player_num: u8,
    pub pass_port: u32,

    // The port exposed to players
    pub external_port: u16,

    // The port to the SC2 process
    // TODO: Use fixed internal port instead
    pub internal_port: u16,
}

impl PlayerSeat {
    pub fn new(num: u8, port: u16) -> Self {
        PlayerSeat {
            player_num: num,
            pass_port: get_pass_port(num),
            external_port: get_external_port(num),
            internal_port: port,
        }
    }
}

fn get_external_port(num: u8) -> u16 {
    let env_var = format!("PLAYER_{}_SEAT", num);
    let value = std::env::var(&env_var).unwrap_or_else(|_| {
        panic!("Missing {} environment variable", env_var);
    });
    value.parse().unwrap_or_else(|_| {
        panic!("Invalid {} environment variable", env_var);
    })
}

fn get_pass_port(num: u8) -> u32 {
    let env_var = format!("PLAYER_{}_PASS", num);
    let value = std::env::var(&env_var).unwrap_or_else(|_| get_external_port(num).to_string());
    value.parse().unwrap_or_else(|_| {
        panic!("Invalid {} environment variable", env_var);
    })
}
