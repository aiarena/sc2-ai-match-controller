use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMap {
    pub name: String,
    pub file: String,
    pub file_hash: Option<String>,
}
