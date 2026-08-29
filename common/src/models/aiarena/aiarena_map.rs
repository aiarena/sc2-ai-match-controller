use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMap {
    pub name: String,
    pub download_link: String,
}
