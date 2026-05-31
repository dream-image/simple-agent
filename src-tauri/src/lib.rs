pub mod context;
pub mod tools;
use crate::context::session_context::SESSION_CONTEXT;
use crate::tools::edit_file::EditFile;
use crate::tools::edit_file::ToolInput as EditFileInput;
use crate::tools::get_weather::ToolInput as GetWeatherInput;
use crate::tools::read_file::ReadFile;
use crate::tools::read_file::ToolInput as ReadFileInput;
use crate::tools::shell::Shell;
use crate::tools::shell::ToolInput as ShellInput;
use crate::tools::Tool;
use anyhow::{anyhow, Context};
use num_format::{Locale, ToFormattedString};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Mutex;
use tools::get_weather::GetWeather;
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;
use eventsource_stream::Eventsource;
#[derive(Default,serde::Serialize, Clone,Debug,Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    #[default]
    User,
    // System,
    Assistant,
    Tool,
}
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Default,Clone, Deserialize, Debug, Serialize)]
struct Function {
    name: String,
    arguments: String,
}
#[derive(Default,Clone, Deserialize, Debug, Serialize)]
struct ToolCall {
    index: u32,
    id: String,
    r#type: String,
    function: Function,
}


#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct Choice {
    finish_reason: String,
    index: u32,
    message: InputMessage,
}

#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct ThinkingDelta {
    reasoning_content: String,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct ContentDelta {
    content: String,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct StreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct ToolDefine {
    index: u32,
    id: Option<String>,
    r#type: Option<String>,
    function: StreamFunction,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct SessionStart {
    role:Role,

}

#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct DeltaRaw {
    role: Option<Role>,
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolDefine>>,
}

#[derive(Default,Debug, Serialize,Clone)]
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
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct StreamChoice {
    index: u32,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct CompletionTokensDetails {
    reasoning_tokens: u32,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct PromptTokensDetails {
    cached_tokens: u32,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
struct Usage {
    completion_tokens: u32,
    completion_tokens_details: CompletionTokensDetails,
    prompt_cache_hit_tokens: u32,
    prompt_cache_miss_tokens: u32,
    prompt_tokens: u32,
    prompt_tokens_details: PromptTokensDetails,
    total_tokens: u32,
}

#[derive(Default,Deserialize, Debug, Serialize)]
struct AiResponse {
    choices: Vec<Choice>,
    id: String,
    model: String,
    object: String,
    usage: Usage,
}
#[derive(Default,Deserialize, Debug, Serialize,Clone)]
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

#[derive(serde::Serialize, Clone, Debug,Deserialize,Default)]
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
}

async fn agent_call(history: &[InputMessage]) -> anyhow::Result<AiResponse> {
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
async fn agent_call_stream(app:&AppHandle,session_id: &str,history: &[InputMessage]) -> anyhow::Result<AiResponse> {
    let body_json = json!({
         "model": "deepseek-v4-pro",
         "messages": history,
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

    let mut events=  response.bytes_stream().eventsource();
    let mut ai_response = AiResponse::default();
    let mut input_message = InputMessage{
        role: Role::Assistant,
        content: None,
        reasoning_content: None,
        tool_calls: None,
        tool_call_id: None,
    };
    let mut finish_reason = "stop".to_string();
    while let Some(event) = events.next().await {
        let event = event?;
        let _ = app.emit("agent_stream",AgentStreamEvent{
            session_id: session_id.to_string(),
            data: event.data.clone(),
        });
        if event.data =="[DONE]"{
            break;
        }
        println!("{:?}", event.data);
        let value = serde_json::from_str::<AiStreamResponse>(&event.data)?;
        ai_response.id=value.id;
        ai_response.model=value.model;
        ai_response.object=value.object;
        if let Some(usage)=value.usage {
            ai_response.usage=usage;
        }

        let Some(choice) = value.choices.first() else {
            continue;
        };
        if let  Some(reason)=choice.finish_reason.clone(){
            finish_reason=reason;
            continue;
        }
        let delta = &choice.delta;
        match delta {
            Delta::SessionStart(data)=>{
                input_message.role = data.role.clone();
            },
            Delta::Thinking(data)=>{
                input_message.reasoning_content.get_or_insert_default().push_str(data.reasoning_content.as_str());
            },
            Delta::Content(data)=>{
                input_message.content.get_or_insert_default().push_str(data.content.as_str());
            },
            Delta::ToolCalls(data)=>{
                let list = input_message.tool_calls.get_or_insert_default();
                for item in data {
                    let tool = if let Some(index) = list.iter().position(|tool| tool.index == item.index) {
                        &mut list[index]
                    } else {
                        list.push(ToolCall {
                            index: item.index,
                            id: item.id.clone().unwrap_or_default(),
                            r#type: item.r#type.clone().unwrap_or_else(|| "function".to_string()),
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
            },
            _ =>{

            }
        }

    };

    ai_response.choices= vec![Choice{
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

static TOOLS_MAP: LazyLock<Mutex<HashMap<String, ToolsEnum>>> =
    LazyLock::new(|| Mutex::new(HashMap::<String, ToolsEnum>::new()));
static HISTORY_MAP: LazyLock<Mutex<HashMap<String, Vec<InputMessage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::<String, Vec<InputMessage>>::new()));
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
async fn create_session(session_id: &str) -> Result<(), String> {
    let mut history_map = HISTORY_MAP.lock().await;
    history_map
        .entry(session_id.to_string())
        .or_insert_with(Vec::new);
    Ok(())
}

#[tauri::command]
async fn delete_session(session_id: &str) -> Result<(), String> {
    let mut history_map = HISTORY_MAP.lock().await;
    history_map.remove(session_id);
    Ok(())
}

#[tauri::command]
async fn call(app:AppHandle,session_id: &str, question: &str) -> Result<InputMessage, String> {
    let mut history = {
        let mut history_map = HISTORY_MAP.lock().await;
        let history = history_map
            .get_mut(session_id)
            .ok_or_else(|| "session不存在".to_string())?;
        history.push(InputMessage {
            role: Role::User,
            content: Some(question.trim_end().to_string()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        });
        history.clone()
    };

    let tool_result_push =
        |history: &mut Vec<InputMessage>, res: anyhow::Result<String>, tool: &ToolCall| {
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
            };
            history.push(message.clone());
            let _ = app.emit("agent_tool_call",AgentToolCallEvent{
                session_id: session_id.to_string(),
                id: tool.id.clone(),
                name: tool.function.name.clone(),
                arguments: tool.function.arguments.clone(),
                content,
                success,
            });
        };
    loop {
        let res = agent_call_stream(&app,session_id,&history).await.map_err(|err| err.to_string())?;
        {
            let mut session_context = SESSION_CONTEXT.lock().await;
            session_context.add_token(res.usage.total_tokens);
            println!(
                "token:{}/{}",
                session_context.token,
                session_context.totalToken.to_formatted_string(&Locale::en)
            );
        }
        let first_content_message = &res
            .choices
            .first()
            .ok_or_else(|| "响应为空".to_string())?
            .message;
        if let Some(tool_calls) = &first_content_message.tool_calls {
            history.push(InputMessage {
                role: Role::Assistant,
                content: first_content_message.content.clone(),
                reasoning_content: first_content_message.reasoning_content.clone(),
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });
            let tools_map = TOOLS_MAP.lock().await;
            for tool in tool_calls {
                let name = tool.function.name.as_str();
                match (name, tools_map.get(name)) {
                    ("get_weather", Some(ToolsEnum::GetWeather(get_weather))) => {
                        let result =
                            serde_json::from_str::<GetWeatherInput>(&tool.function.arguments)
                                .map(|input| get_weather.execute(input))
                                .map_err(|err| anyhow!(err));
                        tool_result_push(&mut history, result, tool);
                    }
                    ("edit_file", Some(ToolsEnum::EditFile(edit_file))) => {
                        let result =
                            serde_json::from_str::<EditFileInput>(&tool.function.arguments)
                                .map_err(|err| anyhow!(err))
                                .and_then(|input| edit_file.execute(input));
                        tool_result_push(&mut history, result, tool);
                    }
                    ("read_file", Some(ToolsEnum::ReadFile(read_file))) => {
                        let result =
                            serde_json::from_str::<ReadFileInput>(&tool.function.arguments)
                                .map_err(|err| anyhow!(err))
                                .and_then(|input| read_file.execute(input));
                        tool_result_push(&mut history, result, tool);
                    }
                    ("shell", Some(ToolsEnum::Shell(shell))) => {
                        let result = serde_json::from_str::<ShellInput>(&tool.function.arguments)
                            .map_err(|err| anyhow!(err))
                            .and_then(|input| shell.execute(input));
                        tool_result_push(&mut history, result, tool);
                    }
                    _ => {
                        tool_result_push(
                            &mut history,
                            anyhow::Ok(format!("该{}工具未实现", &tool.function.name)),
                            tool,
                        );
                        println!("工具：{}，还没有实现", tool.function.name);
                    }
                }
            }
            continue;
        } else if let Some(response) = &first_content_message.content {
            println!("========================================================================");
            println!("{}", response);
            println!("========================================================================");
            let message = InputMessage {
                role: Role::Assistant,
                content: Some(response.clone()),
                reasoning_content: first_content_message.reasoning_content.clone(),
                tool_calls: None,
                tool_call_id: None,
            };
            history.push(message.clone());
            let mut history_map = HISTORY_MAP.lock().await;
            history_map.insert(session_id.to_string(), history);
            return Ok(message.clone());
        }

        return Err("响应为空".to_string());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            agent_init,
            create_session,
            delete_session,
            call
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
