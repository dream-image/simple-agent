use std::sync::{LazyLock};
use tokio::sync::Mutex;

pub struct SessionContext{
    pub token:u32,
    pub totalToken:u32,
}
impl SessionContext{
    pub fn add_token(&mut self, token:u32){
        self.token = token+self.token;
    }
}
pub static SESSION_CONTEXT: LazyLock<Mutex<SessionContext>> =LazyLock::new(||{
    Mutex::new(SessionContext{
        token:0,
        totalToken:100*1000_0,
    })
});