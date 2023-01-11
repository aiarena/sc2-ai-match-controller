use crate::api::process::ProcessMap;
use crate::configuration::ac_config::ACConfig;
use crate::utilities::portpicker::Port;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct AppState {
    pub process_map: ProcessMap,
    pub settings: ACConfig,
    pub shutdown_sender: Sender<()>,
    pub extra_info: Arc<RwLock<HashMap<Port, HashMap<String, String>>>>,
}
