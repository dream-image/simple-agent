use crate::permission::{PermissionMod, ToolsPermission};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

#[derive(Serialize, Clone, Debug, Deserialize, Default)]
pub struct SessionContext {
    pub token: u32,
    pub total_token: u32,
    pub mode: PermissionMod,
    pub permission: ToolsPermission,
}

impl SessionContext {
    pub fn new(token: u32, total_token: u32) -> Self {
        Self {
            token,
            total_token,
            ..Default::default()
        }
    }
    pub fn add_token(&mut self, token: u32) {
        self.token = token + self.token;
    }
    pub fn clear_token(&mut self) {
        self.token = 0;
    }
}

pub static SESSION_CONTEXT_MAP: LazyLock<Mutex<HashMap<String, Arc<Mutex<SessionContext>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
