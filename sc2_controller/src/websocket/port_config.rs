//! Full port configuration

use protobuf::MessageField;

use sc2_proto::sc2api::{PortSet, RequestJoinGame};

/// Full set of ports needed by SC2
#[derive(Debug, Clone)]
pub struct PortConfig {
    shared: u16,
    server_game: u16,
    server_base: u16,
    client_game: u16,
    client_base: u16,
}

impl PortConfig {
    pub fn new() -> Self {
        Self {
            shared: 9101,
            server_game: 9102,
            server_base: 9103,
            client_game: 9104,
            client_base: 9105,
        }
    }

    /// Apply port configuration to a handler join request
    pub fn apply_proto(&self, req: &mut RequestJoinGame) {
        req.set_shared_port(self.shared as i32);

        let mut server_ps = PortSet::new();
        server_ps.set_game_port(self.server_game as i32);
        server_ps.set_base_port(self.server_base as i32);
        req.server_ports = MessageField::from_option(Some(server_ps));

        let mut client_ps = PortSet::new();
        client_ps.set_game_port(self.client_game as i32);
        client_ps.set_base_port(self.client_base as i32);
        req.client_ports = vec![client_ps];
    }
}
