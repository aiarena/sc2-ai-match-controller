use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Arenaclient {
    pub name: String,
    pub token: String,
    #[serde(skip_deserializing)]
    pub allocated: bool,
}
