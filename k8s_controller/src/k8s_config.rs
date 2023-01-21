use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct K8sConfig {
    pub interval_seconds: u64,
    pub old_match_delete_after_minutes: i64,
    pub job_prefix: String,
    pub website_url: String,
    pub namespace: String,
    pub arenaclients_json_path: String,
}
