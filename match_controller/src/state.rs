use common::PlayerNum;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Player {
    addr: SocketAddr,
    player_num: Option<PlayerNum>,
    bot_name: Option<String>,
}

impl Player {
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }
    pub const fn player_num(&self) -> Option<PlayerNum> {
        self.player_num
    }
    pub const fn bot_name(&self) -> Option<&String> {
        self.bot_name.as_ref()
    }
}
