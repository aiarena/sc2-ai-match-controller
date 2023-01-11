use sc2_proto::common::Race;
use sc2_proto::sc2api::RequestJoinGame;

/// Player data, like join parameters
#[derive(Debug, Clone)]
pub struct PlayerData {
    pub race: Race,
    pub name: Option<String>,
    pub interface_options: sc2_proto::sc2api::InterfaceOptions,
}

impl PlayerData {
    pub fn from_join_request(req: &RequestJoinGame) -> Self {
        Self {
            race: req.race(),
            name: if req.has_player_name() {
                Some(req.player_name().to_owned())
            } else {
                None
            },

            interface_options: {
                let mut if_opts = req.options.clone().unwrap();

                if_opts.set_raw_affects_selection(true);
                if_opts.set_raw_crop_to_playable_area(false);
                if_opts
            },
        }
    }
}
