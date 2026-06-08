use crate::tools::Tool;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Listener};
use tokio::sync::oneshot;
use tokio::sync::oneshot::channel;
use crate::{History, SessionStatus};

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
    history: &mut History,
    session_id:&str,
    tool: &T,
    tool_id: &str,
) -> Result<ResponsePayload, String> {
    let (sender, receiver) = channel();
    history.session_status=SessionStatus::Pending;
    println!(
        "[permission] wait session_id={} tool_id={} name={}",
        session_id,
        tool_id,
        T::NAME
    );
    let callback_session_id = session_id.to_string();
    let callback_tool_id = tool_id.to_string();
    app.once(format!("permission_callback_{}_{}",session_id,tool_id), move |event| {
        println!(
            "[permission] callback session_id={} tool_id={} payload={}",
            callback_session_id,
            callback_tool_id,
            event.payload()
        );
        sender.send(event.payload().to_string()).expect("消息发送失败");
    });

    app.emit(
        format!("apply_permission_{}_{}",session_id,tool_id).as_str(),
        RequestPayload {
            name: T::NAME.to_string(),
            session_id: session_id.to_string(),
            content: format!("工具：{}，请求执行权限", T::NAME.to_string()),
        },
    )
    .expect(format!("apply_permission_{}_{}",session_id,tool_id).as_str());
    println!(
        "[permission] emitted session_id={} tool_id={} name={}",
        session_id,
        tool_id,
        T::NAME
    );

    receiver
        .await
        .map(|x| {
            let value: ResponsePayload = serde_json::from_str(&x).expect("权限申请结果反序列化失败");
            println!(
                "[permission] parsed session_id={} tool_id={} result={:?}",
                session_id,
                tool_id,
                value.result
            );
            history.session_status=SessionStatus::Connect;
            value
            
        })
        .map_err(|x| x.to_string())
}
