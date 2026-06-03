use crate::tools::Tool;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Listener};
use tokio::sync::oneshot;
use tokio::sync::oneshot::channel;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct RequestPayload {
    name: String,
    content: String,
    session_id: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PermissionResult {
    AlwaysAllow = 3,
    Allow = 2,
    Deny = 1,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResponsePayload {
    pub result: PermissionResult,
    pub reason: String,
}
// Rust
#[tauri::command]
pub async fn wait_permission_apply<T: Tool>(
    app: &tauri::AppHandle,
    session_id:&str,
    tool: &T,
) -> Result<ResponsePayload, String> {
    let (sender, receiver) = channel();

    app.once("permission_callback", move |event| {
        sender.send(event.payload().to_string()).expect("消息发送失败");
    });

    app.emit(
        "apply_permission",
        RequestPayload {
            name: T::NAME.to_string(),
            session_id: session_id.to_string(),
            content: format!("工具：{}，请求执行权限", T::NAME.to_string()),
        },
    )
    .expect("apply_permission请求失败");

    receiver
        .await
        .map(|x| {
            let value: ResponsePayload = serde_json::from_str(&x).expect("权限申请结果反序列化失败");
            value
        })
        .map_err(|x| x.to_string())
}
