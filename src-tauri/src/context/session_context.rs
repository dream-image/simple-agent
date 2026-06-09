use crate::permission::{PermissionMod, ToolsPermission};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::{watch, Mutex};
use crate::context::workspace::Workspace;

#[derive(serde::Serialize, Clone, Debug, Deserialize, Default, Ord, PartialOrd, PartialEq, Eq,Copy)]
pub enum SessionStatus {
    #[default]
    Default,
    Pending,
    Connect,
    Stop,
}

#[derive(Clone,Debug)]
pub struct SessionStatusState{
    tx:watch::Sender<SessionStatus>,
}

impl SessionStatusState{
    pub fn new(status:SessionStatus) -> Self {
        let (tx,_) = watch::channel(status);
        SessionStatusState{tx}
    }
    pub fn get(&self) -> SessionStatus {
        self.tx.borrow().clone()
    }
    pub fn set(&self, status:SessionStatus) {
        self.tx.send_replace(status);
    }
    pub fn subscribe(&self)->watch::Receiver<SessionStatus> {
        self.tx.subscribe()
    }
    pub fn is_stop(&self) -> bool {
        self.get() == SessionStatus::Stop
    }

}

impl Default for SessionStatusState{
    fn default() -> Self {
        SessionStatusState::new(SessionStatus::Default)
    }
}
impl<'d> Deserialize<'d> for SessionStatusState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'d>
    {
        Ok(Self::new(SessionStatus::deserialize(deserializer)?))
    }
}

impl Serialize for SessionStatusState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        self.get().serialize(serializer)
    }
}


#[derive(Serialize, Clone, Debug, Deserialize, Default)]
pub struct SessionContext {
    pub token: u32,
    pub total_token: u32,
    pub mode: PermissionMod,
    pub permission: ToolsPermission,
    pub workspace: Workspace,
    pub session_status: SessionStatusState,
}




impl SessionContext {
    pub fn new(token: Option<u32>, total_token: Option<u32>) -> Self {
        let token = if let Some(token) = token {
            token
        }else {
            0
        };
        let total_token = if let Some(total_token) = total_token {
            total_token
        }else{
            100_0000
        };
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
