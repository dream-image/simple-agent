pub mod context;
mod event;
mod path;
mod permission;
mod prompt;
pub mod tools;

use crate::context::session_context::{SessionContext, SessionStatus, SessionStatusState, SESSION_CONTEXT_MAP};
use crate::context::workspace::Workspace;
use crate::event::{wait_permission_apply, PermissionResult};
use crate::path::{init_data_dir, write_file};
use crate::permission::{check_final_permission, Permission, PermissionLevel, PermissionMod};
use crate::prompt::get_system_prompt;
use crate::tools::edit_file::EditFile;
use crate::tools::edit_file::ToolInput as EditFileInput;
use crate::tools::get_weather::ToolInput as GetWeatherInput;
use crate::tools::read_file::ReadFile;
use crate::tools::read_file::ToolInput as ReadFileInput;
use crate::tools::shell::Shell;
use crate::tools::shell::ToolInput as ShellInput;
use crate::tools::Tool;
use anyhow::{anyhow, Context};
use eventsource_stream::Eventsource;
use num_format::{Locale, ToFormattedString};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Mutex, MutexGuard};
use tokio_stream::StreamExt;
use tools::get_weather::GetWeather;

#[derive(Default, serde::Serialize, Clone, Debug, Deserialize, Ord, PartialOrd, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Role {
    #[default]
    User,
    System,
    Assistant,
    Tool,
    UserStop,
}
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Default, Clone, Deserialize, Debug, Serialize)]
struct Function {
    name: String,
    arguments: String,
}
#[derive(Default, Clone, Deserialize, Debug, Serialize)]
struct ToolCall {
    index: u32,
    id: String,
    r#type: String,
    function: Function,
}

#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct Choice {
    finish_reason: String,
    index: u32,
    message: InputMessage,
}

#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct ThinkingDelta {
    reasoning_content: String,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct ContentDelta {
    content: String,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct StreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct ToolDefine {
    index: u32,
    id: Option<String>,
    r#type: Option<String>,
    function: StreamFunction,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct SessionStart {
    role: Role,
}

#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct DeltaRaw {
    role: Option<Role>,
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolDefine>>,
}

#[derive(Default, Debug, Serialize, Clone)]
enum Delta {
    #[default]
    Empty,
    SessionStart(SessionStart),
    Thinking(ThinkingDelta),
    Content(ContentDelta),
    ToolCalls(Vec<ToolDefine>),
}
impl<'de> Deserialize<'de> for Delta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = DeltaRaw::deserialize(deserializer)?;
        Ok(raw.into())
    }
}
impl From<DeltaRaw> for Delta {
    fn from(raw: DeltaRaw) -> Self {
        if let Some(tool_calls) = raw.tool_calls {
            if !tool_calls.is_empty() {
                return Delta::ToolCalls(tool_calls);
            }
        }
        if let Some(reasoning_content) = raw.reasoning_content {
            if !reasoning_content.is_empty() {
                return Delta::Thinking(ThinkingDelta { reasoning_content });
            }
        }
        if let Some(content) = raw.content {
            if !content.is_empty() {
                return Delta::Content(ContentDelta { content });
            }
        }
        if let Some(role) = raw.role {
            return Delta::SessionStart(SessionStart { role });
        }
        Delta::Empty
    }
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct StreamChoice {
    index: u32,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct CompletionTokensDetails {
    reasoning_tokens: u32,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct PromptTokensDetails {
    cached_tokens: u32,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct Usage {
    completion_tokens: u32,
    completion_tokens_details: CompletionTokensDetails,
    prompt_cache_hit_tokens: u32,
    prompt_cache_miss_tokens: u32,
    prompt_tokens: u32,
    prompt_tokens_details: PromptTokensDetails,
    total_tokens: u32,
}

#[derive(Default, Deserialize, Debug, Serialize)]
struct AiResponse {
    choices: Vec<Choice>,
    id: String,
    model: String,
    object: String,
    usage: Usage,
}
#[derive(Default, Deserialize, Debug, Serialize, Clone)]
struct AiStreamResponse {
    choices: Vec<StreamChoice>,
    id: String,
    model: String,
    object: String,
    usage: Option<Usage>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgentStreamEvent {
    session_id: String,
    data: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgentToolCallEvent {
    session_id: String,
    id: String,
    name: String,
    arguments: String,
    content: String,
    success: bool,
}

#[derive(serde::Serialize, Clone, Debug, Deserialize, Default)]
struct InputMessage {
    role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_context: Option<SessionContext>,
}

#[derive(serde::Serialize, Clone, Debug, Deserialize, Default)]
struct ChatMessage {
    role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl From<&InputMessage> for ChatMessage {
    fn from(value: &InputMessage) -> Self {
        Self {
            role: value.role.clone(),
            content: value.content.clone(),
            reasoning_content: value.reasoning_content.clone(),
            tool_calls: value.tool_calls.clone(),
            tool_call_id: value.tool_call_id.clone(),
        }
    }
}

async fn agent_call(history: &[ChatMessage]) -> anyhow::Result<AiResponse> {
    let body_json = json!({
         "model": "deepseek-v4-pro",
         "messages": history,
        "tools":to_value(vec![
            GetWeather::definition(),
            EditFile::definition(),
            ReadFile::definition(),
            Shell::definition(),
        ]).unwrap()
    });
    let client = Client::new();
    // println!(
    //     "入参：{}",
    //     serde_json::to_string_pretty(&body_json).unwrap()
    // );
    // println!("准备发送");

    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .timeout(Duration::from_secs(30))
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            "Bearer sk-55d4dad8ca5e4f7fb95b24d952a55504",
        )
        .json(&body_json)
        .send()
        .await?
        .error_for_status()?;

    // println!("已收到 status: {}", response.status());

    let text = response.text().await?;

    let res = serde_json::from_str::<AiResponse>(&text)
        .with_context(|| format!("响应解析失败，响应体：{}", text))?;
    Ok(res)
}
async fn agent_call_stream(
    app: &AppHandle,
    session_id: &str,
    history: &[InputMessage],
) -> anyhow::Result<AiResponse> {
    let filter_role = vec![Role::Tool, Role::User, Role::System, Role::Assistant];
    let chat_message: Vec<ChatMessage> = history
        .iter()
        .map(ChatMessage::from)
        .filter(|x| filter_role.contains(&x.role))
        .collect();
    let body_json = json!({
         "model": "deepseek-v4-pro",
         "messages": chat_message,
         "tools":to_value(
            vec![
                GetWeather::definition(),
                EditFile::definition(),
                ReadFile::definition(),
                Shell::definition(),
            ],
        )?,
        "stream":true,
        "stream_options":{
            "include_usage":true
        }
    });
    let response = Client::new()
        .post("https://api.deepseek.com/chat/completions")
        .timeout(Duration::from_secs(30))
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            "Bearer sk-55d4dad8ca5e4f7fb95b24d952a55504",
        )
        .json(&body_json)
        .send()
        .await?
        .error_for_status()?;

    let mut events = response.bytes_stream().eventsource();
    let mut ai_response = AiResponse::default();
    let mut input_message = InputMessage {
        role: Role::Assistant,
        content: None,
        reasoning_content: None,
        tool_calls: None,
        tool_call_id: None,
        session_context: None,
    };
    let mut finish_reason = "stop".to_string();
    while let Some(event) = events.next().await {
        let event = event?;
        let _ = app.emit(
            "agent_stream",
            AgentStreamEvent {
                session_id: session_id.to_string(),
                data: event.data.clone(),
            },
        );
        if event.data == "[DONE]" {
            break;
        }
        // println!("{:?}", event.data);
        let value = serde_json::from_str::<AiStreamResponse>(&event.data)?;
        ai_response.id = value.id;
        ai_response.model = value.model;
        ai_response.object = value.object;
        if let Some(usage) = value.usage {
            ai_response.usage = usage;
        }

        let Some(choice) = value.choices.first() else {
            continue;
        };
        if let Some(reason) = choice.finish_reason.clone() {
            finish_reason = reason;
            continue;
        }
        let delta = &choice.delta;
        match delta {
            Delta::SessionStart(data) => {
                input_message.role = data.role.clone();
            }
            Delta::Thinking(data) => {
                input_message
                    .reasoning_content
                    .get_or_insert_default()
                    .push_str(data.reasoning_content.as_str());
            }
            Delta::Content(data) => {
                input_message
                    .content
                    .get_or_insert_default()
                    .push_str(data.content.as_str());
            }
            Delta::ToolCalls(data) => {
                let list = input_message.tool_calls.get_or_insert_default();
                for item in data {
                    let tool = if let Some(index) =
                        list.iter().position(|tool| tool.index == item.index)
                    {
                        &mut list[index]
                    } else {
                        list.push(ToolCall {
                            index: item.index,
                            id: item.id.clone().unwrap_or_default(),
                            r#type: item
                                .r#type
                                .clone()
                                .unwrap_or_else(|| "function".to_string()),
                            function: Default::default(),
                        });
                        list.last_mut().unwrap()
                    };
                    if let Some(id) = &item.id {
                        tool.id = id.clone();
                    }
                    if let Some(r#type) = &item.r#type {
                        tool.r#type = r#type.clone();
                    }
                    if let Some(name) = &item.function.name {
                        tool.function.name = name.clone();
                    }
                    if let Some(arguments) = &item.function.arguments {
                        tool.function.arguments.push_str(arguments);
                    }
                }
            }
            _ => {}
        }
    }

    ai_response.choices = vec![Choice {
        finish_reason,
        index: 0,
        message: input_message,
    }];
    Ok(ai_response)
}

enum ToolsEnum {
    GetWeather(GetWeather),
    EditFile(EditFile),
    ReadFile(ReadFile),
    Shell(Shell),
}

#[derive(serde::Serialize, Clone, Debug, Deserialize, Default)]
pub struct History {
    history: Vec<InputMessage>,
}

static TOOLS_MAP: LazyLock<Mutex<HashMap<String, ToolsEnum>>> =
    LazyLock::new(|| Mutex::new(HashMap::<String, ToolsEnum>::new()));
static HISTORY_MAP: LazyLock<Mutex<HashMap<String, Arc<Mutex<History>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::<String, Arc<Mutex<History>>>::new()));
#[tauri::command]
async fn agent_init() -> Result<(), String> {
    let mut tools_map = TOOLS_MAP.lock().await;
    tools_map.insert(
        String::from("get_weather"),
        ToolsEnum::GetWeather(GetWeather {}),
    );
    tools_map.insert(String::from("edit_file"), ToolsEnum::EditFile(EditFile {}));
    tools_map.insert(String::from("read_file"), ToolsEnum::ReadFile(ReadFile {}));
    tools_map.insert(String::from("shell"), ToolsEnum::Shell(Shell {}));

    Ok(())
}

#[tauri::command]
async fn create_session(session_id: &str, project_dir: &str) -> Result<SessionContext, String> {
    let session_context_value =SessionContext {
        token: 0,
        total_token: 100_0000,
        mode: Default::default(),
        permission: Default::default(),
        workspace: Workspace {
            cwd: project_dir.to_string(),
            project_root: project_dir.to_string(),
            read_root: project_dir.to_string(),
            write_root: project_dir.to_string(),
        },
        session_status: Default::default(),
    };

    let mut history_map = HISTORY_MAP.lock().await;
    history_map
        .entry(session_id.to_string())
        .or_insert_with(|| {
            Arc::new(Mutex::new(History {
                history: vec![InputMessage {
                    role: Role::System,
                    content: Some(get_system_prompt(Some(&session_context_value))),
                    reasoning_content: None,
                    tool_calls: None,
                    tool_call_id: None,
                    session_context: None,
                }],
            }))
        });
    let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
    let session_context=Arc::new(Mutex::new(session_context_value.clone()));
    let _ = session_context_map
        .entry(session_id.to_string())
        .or_insert(session_context)
        .clone();

    Ok(session_context_value)
}

#[tauri::command]
async fn delete_session(session_id: &str) -> Result<(), String> {
    let mut history_map = HISTORY_MAP.lock().await;
    history_map.remove(session_id);
    let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
    session_context_map.remove(session_id);
    Ok(())
}

#[tauri::command]
async fn set_permission_mode(
    session_id: &str,
    mode: PermissionMod,
) -> Result<SessionContext, String> {
    let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
    let session_context = session_context_map
        .entry(session_id.to_string())
        .or_insert(Arc::new(Mutex::new(SessionContext::new(None, None))))
        .clone();
    let mut session_context = session_context.lock().await;
    session_context.mode = mode;
    Ok(session_context.clone())
}

#[tauri::command]
async fn call_cancel(session_id: &str) -> Result<(), String> {
    println!("========================");
    println!("call cancel被调用了");
    let session_context_map = SESSION_CONTEXT_MAP.lock().await;
    let session_context = session_context_map.get(&session_id.to_string());

    if let Some(session) = session_context {
        let session = session.lock().await;
        session.session_status.set(SessionStatus::Stop);

        println!(
            "session_status stop信号发布成功\
        ========================"
        );
        Ok(())
    } else {
        println!("========================");
        Err(format!("No history for session {}", session_id))
    }
}

enum WaitResult {
    Stop(anyhow::Result<()>),
    Continue(anyhow::Result<AiResponse>),
}
async fn wait_stop_or_agent(
    session_context: &mut SessionContext,
    app: &AppHandle,
    session_id: &str,
    history: &[InputMessage],
) -> WaitResult {
    if session_context.session_status.is_stop() {
        return WaitResult::Stop(Ok(()));
    }
    let mut status_rx=session_context.session_status.subscribe();
    let stream = agent_call_stream(app, session_id, history);
    tokio::pin!(stream);
    loop{
        tokio::select! {
        changed = status_rx.changed() => {
            match changed {
                 Err(err)=>{
                     return  WaitResult::Stop( Err(anyhow!(err.to_string())))
                }
                Ok(v) =>{
                    if *status_rx.borrow()== SessionStatus::Stop{
                         return  WaitResult::Stop(Ok(()))
                    }

                }
            }
        }
        stream = &mut stream => {
           return WaitResult::Continue(stream)
        }
    }
    }

}
#[tauri::command]
async fn call(app: AppHandle, session_id: &str, question: &str) -> Result<InputMessage, String> {
    let mut history = {
        let mut history_map = HISTORY_MAP.lock().await;
        let mut history = history_map
            .get_mut(session_id)
            .ok_or_else(|| "session不存在".to_string())?
            .lock()
            .await
            .clone();

        history.history.push(InputMessage {
            role: Role::User,
            content: Some(question.trim_end().to_string()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            session_context: None,
        });

        history
    };
    {
        let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
        let _ =session_context_map
            .entry(session_id.to_string())
            .or_insert(Arc::new(Mutex::new(SessionContext::new(None, None))));

    };

    let tool_result_push = async |session_context: &SessionContext,
                                  history: &mut Vec<InputMessage>,
                                  res: anyhow::Result<String>,
                                  tool: &ToolCall| {
        let (content, success) = match res {
            Ok(value) => (value, true),
            Err(err) => (err.to_string(), false),
        };
        let message = InputMessage {
            role: Role::Tool,
            content: Some(content.clone()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(tool.id.clone()),
            session_context: Some(session_context.clone()),
        };
        history.push(message.clone());
        let _ = app.emit(
            "agent_tool_call",
            AgentToolCallEvent {
                session_id: session_id.to_string(),
                id: tool.id.clone(),
                name: tool.function.name.clone(),
                arguments: tool.function.arguments.clone(),
                content,
                success,
            },
        );
    };
    let mut session_context = {
        let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
        let session_context=session_context_map
            .entry(session_id.to_string())
            .or_insert(Arc::new(Mutex::new(SessionContext::new(None, None))))
            .clone();
        let session_context = session_context.lock().await;
        session_context.session_status.set(SessionStatus::Connect);
        session_context.clone()
    };
    loop {

        let wait_result ={
            wait_stop_or_agent(
                &mut session_context,
                &app,
                session_id,
                &history.history,
            ).await
        };
        // let mut session_context = session_context.clone();
        return match wait_result {
            WaitResult::Continue(result) => {
                let res = result
                    .map_err(|err| err.to_string())?;
                {
                    session_context.add_token(res.usage.total_tokens);
                    println!(
                        "token:{}/{}",
                        &session_context.token,
                        &session_context.total_token.to_formatted_string(&Locale::en)
                    );
                }
                let first_content_message = &res
                    .choices
                    .first()
                    .ok_or_else(|| "响应为空".to_string())?
                    .message;
                if let Some(tool_calls) = &first_content_message.tool_calls {

                    history.history.push(InputMessage {
                        role: Role::Assistant,
                        content: first_content_message.content.clone(),
                        reasoning_content: first_content_message.reasoning_content.clone(),
                        tool_calls: Some(tool_calls.clone()),
                        tool_call_id: None,
                        session_context: Some(session_context.clone()),
                    });
                    let tools_map = TOOLS_MAP.lock().await;
                    for tool in tool_calls {
                        let name = tool.function.name.as_str();
                        let tool_id = tool.id.as_str();
                        let tool_pre_permission =
                            session_context.permission.check_permissions(name);
                        match (name, tools_map.get(name)) {
                            ("get_weather", Some(ToolsEnum::GetWeather(get_weather))) => {
                                let input = serde_json::from_str::<GetWeatherInput>(
                                    &tool.function.arguments,
                                )
                                .map_err(|err| anyhow!(err));
                                if let Ok(input) = input {
                                    let tool_permission = get_weather
                                        .check_permission(&input, &session_context);
                                    let final_permission = check_final_permission(
                                        &tool_pre_permission,
                                        &tool_permission,
                                        &session_context.mode,
                                        get_weather,
                                    );
                                    match final_permission.0 {
                                        PermissionLevel::Deny => {
                                            tool_result_push(
                                                &session_context,
                                                &mut history.history,
                                                Err(anyhow!(final_permission.1)),
                                                tool,
                                            )
                                            .await;
                                            continue;
                                        }
                                        PermissionLevel::Ask => {
                                            let result = wait_permission_apply(
                                                &app,
                                                &session_context,
                                                session_id,
                                                get_weather,
                                                tool_id,
                                            )
                                            .await;
                                            match result {
                                                Ok(value) => match value.result {
                                                    PermissionResult::AlwaysAllow => {

                                                        session_context
                                                            .permission
                                                            .ask_tools
                                                            .remove(name);
                                                        session_context
                                                            .permission
                                                            .allow_tools
                                                            .insert(name.to_string());
                                                    }
                                                    PermissionResult::Deny => {
                                                        tool_result_push(
                                                            &session_context,
                                                            &mut history.history,
                                                            Err(anyhow!(value.reason)),
                                                            tool,
                                                        )
                                                        .await;
                                                        continue;
                                                    }
                                                    _ => {}
                                                },
                                                Err(err) => {
                                                    tool_result_push(
                                                        &session_context,
                                                        &mut history.history,
                                                        Err(anyhow!(err)),
                                                        tool,
                                                    )
                                                    .await;
                                                    continue;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                    let result =
                                        get_weather.execute(input, &session_context);

                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        Ok(result),
                                        tool,
                                    )
                                    .await;
                                } else if let Err(err) = input {
                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        Err(err),
                                        tool,
                                    )
                                    .await;
                                }
                            }
                            ("edit_file", Some(ToolsEnum::EditFile(edit_file))) => {
                                let input =
                                    serde_json::from_str::<EditFileInput>(&tool.function.arguments)
                                        .map_err(|err| anyhow!(err));
                                if let Ok(input) = input {
                                    let tool_permission = edit_file
                                        .check_permission(&input, &session_context);
                                    let final_permission = check_final_permission(
                                        &tool_pre_permission,
                                        &tool_permission,
                                        &session_context.mode,
                                        edit_file,
                                    );
                                    match final_permission.0 {
                                        PermissionLevel::Deny => {
                                            tool_result_push(
                                                &session_context,
                                                &mut history.history,
                                                Err(anyhow!(final_permission.1)),
                                                tool,
                                            )
                                            .await;
                                            continue;
                                        }
                                        PermissionLevel::Ask => {
                                            let result = wait_permission_apply(
                                                &app,
                                                &session_context,
                                                session_id,
                                                edit_file,
                                                tool_id,
                                            )
                                            .await;
                                            match result {
                                                Ok(value) => match value.result {
                                                    PermissionResult::AlwaysAllow => {
                                                        session_context
                                                            .permission
                                                            .ask_tools
                                                            .remove(name);
                                                        session_context
                                                            .permission
                                                            .allow_tools
                                                            .insert(name.to_string());
                                                    }
                                                    PermissionResult::Deny => {
                                                        tool_result_push(
                                                            &session_context,
                                                            &mut history.history,
                                                            Err(anyhow!(value.reason)),
                                                            tool,
                                                        )
                                                        .await;
                                                        continue;
                                                    }
                                                    _ => {}
                                                },
                                                Err(err) => {
                                                    tool_result_push(
                                                        &session_context,
                                                        &mut history.history,
                                                        Err(anyhow!(err)),
                                                        tool,
                                                    )
                                                    .await;
                                                    continue;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                    let result =
                                        edit_file.execute(input, &session_context);

                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        result,
                                        tool,
                                    )
                                    .await;
                                } else if let Err(err) = input {
                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        Err(err),
                                        tool,
                                    )
                                    .await;
                                }
                            }
                            ("read_file", Some(ToolsEnum::ReadFile(read_file))) => {
                                let input =
                                    serde_json::from_str::<ReadFileInput>(&tool.function.arguments)
                                        .map_err(|err| anyhow!(err));
                                if let Ok(input) = input {
                                    let tool_permission = read_file
                                        .check_permission(&input, &session_context);
                                    let final_permission = check_final_permission(
                                        &tool_pre_permission,
                                        &tool_permission,
                                        &session_context.mode,
                                        read_file,
                                    );
                                    match final_permission.0 {
                                        PermissionLevel::Deny => {
                                            tool_result_push(
                                                &session_context,
                                                &mut history.history,
                                                Err(anyhow!(final_permission.1)),
                                                tool,
                                            )
                                            .await;
                                            continue;
                                        }
                                        PermissionLevel::Ask => {
                                            let result = wait_permission_apply(
                                                &app,
                                                &mut session_context,
                                                session_id,
                                                read_file,
                                                tool_id,
                                            )
                                            .await;
                                            match result {
                                                Ok(value) => match value.result {
                                                    PermissionResult::AlwaysAllow => {

                                                        session_context
                                                            .permission
                                                            .ask_tools
                                                            .remove(name);
                                                        session_context
                                                            .permission
                                                            .allow_tools
                                                            .insert(name.to_string());
                                                    }
                                                    PermissionResult::Deny => {
                                                        tool_result_push(
                                                            &session_context,
                                                            &mut history.history,
                                                            Err(anyhow!(value.reason)),
                                                            tool,
                                                        )
                                                        .await;
                                                        continue;
                                                    }
                                                    _ => {}
                                                },
                                                Err(err) => {
                                                    tool_result_push(
                                                        &session_context,
                                                        &mut history.history,
                                                        Err(anyhow!(err)),
                                                        tool,
                                                    )
                                                    .await;
                                                    continue;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                    let result =
                                        read_file.execute(input, &session_context);

                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        result,
                                        tool,
                                    )
                                    .await;
                                } else if let Err(err) = input {
                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        Err(err),
                                        tool,
                                    )
                                    .await;
                                }
                            }
                            ("shell", Some(ToolsEnum::Shell(shell))) => {
                                let input =
                                    serde_json::from_str::<ShellInput>(&tool.function.arguments)
                                        .map_err(|err| anyhow!(err));
                                if let Ok(input) = input {
                                    let tool_permission =
                                        shell.check_permission(&input, &session_context);
                                    let final_permission = check_final_permission(
                                        &tool_pre_permission,
                                        &tool_permission,
                                        &session_context.mode,
                                        shell,
                                    );
                                    match final_permission.0 {
                                        PermissionLevel::Deny => {
                                            tool_result_push(
                                                &session_context,
                                                &mut history.history,
                                                Err(anyhow!(final_permission.1)),
                                                tool,
                                            )
                                            .await;
                                            continue;
                                        }
                                        PermissionLevel::Ask => {
                                            let result = wait_permission_apply(
                                                &app,
                                                &mut session_context,
                                                session_id,
                                                shell,
                                                tool_id,
                                            )
                                            .await;
                                            match result {
                                                Ok(value) => match value.result {
                                                    PermissionResult::AlwaysAllow => {

                                                        session_context
                                                            .permission
                                                            .ask_tools
                                                            .remove(name);
                                                        session_context
                                                            .permission
                                                            .allow_tools
                                                            .insert(name.to_string());
                                                    }
                                                    PermissionResult::Deny => {
                                                        tool_result_push(
                                                            &session_context,
                                                            &mut history.history,
                                                            Err(anyhow!(value.reason)),
                                                            tool,
                                                        )
                                                        .await;
                                                        continue;
                                                    }
                                                    _ => {}
                                                },
                                                Err(err) => {
                                                    tool_result_push(
                                                        &session_context,
                                                        &mut history.history,
                                                        Err(anyhow!(err)),
                                                        tool,
                                                    )
                                                    .await;
                                                    continue;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                    let result = shell.execute(input, &session_context);

                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        result,
                                        tool,
                                    )
                                    .await;
                                } else if let Err(err) = input {
                                    tool_result_push(
                                        &session_context,
                                        &mut history.history,
                                        Err(err),
                                        tool,
                                    )
                                    .await;
                                }
                            }
                            _ => {
                                tool_result_push(
                                    &session_context,
                                    &mut history.history,
                                    anyhow::Ok(format!("该{}工具未实现", &tool.function.name)),
                                    tool,
                                )
                                .await;
                                println!("工具：{}，还没有实现", tool.function.name);
                            }
                        }
                    }
                    continue;
                } else if let Some(response) = &first_content_message.content {
                    println!(
                        "========================================================================"
                    );
                    println!("{}", response);
                    println!(
                        "========================================================================"
                    );

                    let message = InputMessage {
                        role: Role::Assistant,
                        content: Some(response.clone()),
                        reasoning_content: first_content_message.reasoning_content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        session_context: Some(session_context.clone()),
                    };
                    history.history.push(message.clone());
                    let mut history_map = HISTORY_MAP.lock().await;
                    history_map.insert(
                        session_id.to_string(),
                        Arc::new(Mutex::new(history.clone())),
                    );
                    let write_result = write_file(
                        &app,
                        format!("{}.jsonl", session_id.to_string()).as_str(),
                        serde_json::to_string_pretty(&history.clone()).unwrap(),
                    );
                    if let Err(err) = write_result {
                        println!("会话记录永久性存储失败：{}", err)
                    }
                    session_context.session_status.set(SessionStatus::Default);
                    let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
                    session_context_map.insert(session_id.to_string(), Arc::new(Mutex::new(session_context.clone())));
                    return Ok(message.clone());
                }

                Err("响应为空".to_string())
            }
            WaitResult::Stop(result) => {
                println!(
                    "========================================================================"
                );
                println!("中止");
                println!(
                    "========================================================================"
                );
                let message = InputMessage {
                    role: Role::Assistant,
                    content: Some("被用户手动中止".to_string()),
                    reasoning_content: None,
                    tool_calls: None,
                    tool_call_id: None,
                    session_context: Some(session_context.clone()),
                };
                history.history.push(message.clone());
                let mut history_map = HISTORY_MAP.lock().await;
                history_map.insert(
                    session_id.to_string(),
                    Arc::new(Mutex::new(history.clone())),
                );
                let write_result = write_file(
                    &app,
                    format!("{}.jsonl", session_id.to_string()).as_str(),
                    serde_json::to_string_pretty(&history.clone()).unwrap(),
                );
                if let Err(err) = write_result {
                    println!("会话记录永久性存储失败：{}", err)
                }
                session_context.session_status.set(SessionStatus::Default);
                let mut session_context_map = SESSION_CONTEXT_MAP.lock().await;
                session_context_map.insert(session_id.to_string(), Arc::new(Mutex::new(session_context.clone())));
                Ok(message.clone())
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let res = init_data_dir(&app_handle);
            if res.is_err() {
                println!("初始化数据仓库失败：{:?}", res);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            agent_init,
            create_session,
            delete_session,
            set_permission_mode,
            call,
            call_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
