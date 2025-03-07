//! Full port configuration

use common::utilities::portpicker::pick_unused_port_in_range;
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
    /// Create a set of random ports
    pub fn new() -> Option<Self> {
        Some(Self {
            shared: pick_unused_port_in_range(9000..10000)?,
            server_game: pick_unused_port_in_range(9000..10000)?,
            server_base: pick_unused_port_in_range(9000..10000)?,
            client_game: pick_unused_port_in_range(9000..10000)?,
            client_base: pick_unused_port_in_range(9000..10000)?,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use sc2_proto::sc2api::RequestJoinGame;

    #[test]
    fn test_portconfig() {
        let mut request = RequestJoinGame::new();
        let port_config = PortConfig::new().expect("Could not create port configuration");
        port_config.apply_proto(&mut request);
        assert!(request.server_ports.is_some());
        assert!(request.has_shared_port());
    }
}
