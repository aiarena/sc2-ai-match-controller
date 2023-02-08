use crate::game::game_config::GameConfig;
use crate::game::game_result::GameResult;
use crate::matches::Match;
use crate::websocket::port_config::PortConfig;
use common::api::api_reference::bot_controller_client::BotController;
use common::api::api_reference::sc2_controller_client::SC2Controller;
use common::configuration::ac_config::ACConfig;
use common::models::StartResponse;
use common::utilities::portpicker::Port;
use common::PlayerNum;
use indexmap::IndexSet;
use std::net::SocketAddr;
use tokio::sync::mpsc::Sender;

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

#[derive(Debug, Clone)]
pub struct ProxyState {
    pub settings: ACConfig,
    pub players: Vec<Player>,
    pub current_match: Option<Match>,
    pub game_config: Option<GameConfig>,
    pub sc2_urls: Vec<SC2Url>,
    pub map: Option<String>,
    pub ready: bool,
    pub port_config: Option<PortConfig>,
    pub game_result: Option<GameResult>,
    pub auth_whitelist: IndexSet<SocketAddr>,
    pub shutdown_sender: Sender<()>,
    pub bot_controllers: Vec<BotController>,
    pub sc2_controllers: Vec<SC2Controller>,
}

impl ProxyState {
    pub fn add_client(&mut self, addr: SocketAddr) {
        self.players.push(Player {
            addr,
            player_num: None,
            bot_name: None,
        });
    }
    pub fn remove_client(&mut self, addr: SocketAddr) -> Option<Player> {
        self.players
            .iter()
            .position(|x| x.addr() == addr)
            .map(|index| self.players.swap_remove(index))
    }
    pub fn get_player_details(&self, addr: SocketAddr) -> Option<Player> {
        self.players.iter().find(|x| x.addr == addr).cloned()
    }
    pub fn remove_all_clients(&mut self) {
        self.players.clear();
    }

    pub fn update_player(&mut self, port: Port, bot_name: &str, player_num: PlayerNum) -> bool {
        if let Some(player) = self.players.iter_mut().find(|x| x.addr.port() == port) {
            player.player_num = Some(player_num);
            player.bot_name = Some(bot_name.to_string());
            true
        } else {
            false
        }
    }
    pub fn get_free_sc2_url(&mut self) -> Option<SC2Url> {
        if let Some(sc2_url) = { self.sc2_urls.iter_mut().find(|x| !x.is_allocated) } {
            sc2_url.is_allocated = true;
            Some(sc2_url.clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct SC2Url {
    pub host: String,
    pub port: Port,
    pub is_allocated: bool,
}

impl SC2Url {
    pub fn new(host: &str, start_response: &StartResponse) -> Self {
        Self {
            host: host.to_string(),
            port: start_response.port,
            is_allocated: false,
        }
    }
}
