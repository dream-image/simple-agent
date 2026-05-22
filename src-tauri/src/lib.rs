pub mod context;
pub mod tools;
use crate::context::session_context::SESSION_CONTEXT;
use crate::tools::edit_file::EditFile;
use crate::tools::edit_file::ToolInput as EditFileInput;
use crate::tools::get_weather::ToolInput as GetWeatherInput;
use crate::tools::read_file::ReadFile;
use crate::tools::read_file::ToolInput as ReadFileInput;
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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Clone, Deserialize, Debug, Serialize)]
struct Function {
    name: String,
    arguments: String,
}
#[derive(Clone, Deserialize, Debug, Serialize)]
struct ToolCall {
    index: u32,
    id: String,
    r#type: String,
    function: Function,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
struct Message {
    content: Option<String>,
    reasoning_content: Option<String>,
    role: String,
    tool_calls: Option<Vec<ToolCall>>,
}
#[derive(Deserialize, Debug, Serialize)]
struct Choice {
    finish_reason: String,
    index: u32,
    message: Message,
}
#[derive(Deserialize, Debug, Serialize)]
struct CompletionTokensDetails {
    reasoning_tokens: u32,
}
#[derive(Deserialize, Debug, Serialize)]
struct PromptTokensDetails {
    cached_tokens: u32,
}
#[derive(Deserialize, Debug, Serialize)]
struct Usage {
    completion_tokens: u32,
    completion_tokens_details: CompletionTokensDetails,
    prompt_cache_hit_tokens: u32,
    prompt_cache_miss_tokens: u32,
    prompt_tokens: u32,
    prompt_tokens_details: PromptTokensDetails,
    total_tokens: u32,
}

#[derive(Deserialize, Debug, Serialize)]
struct AiResponse {
    choices: Vec<Choice>,
    id: String,
    model: String,
    object: String,
    usage: Usage,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    // System,
    Assistant,
    Tool,
}
#[derive(serde::Serialize,Clone)]
struct InputMessage {
    role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize)]
struct AgentReply {
    content: String,
    reasoning_content: Option<String>,
}

async fn agent_call(history: &[InputMessage]) -> anyhow::Result<AiResponse> {
    let body_json = json!({
         "model": "deepseek-v4-pro",
         "messages": history,
        "tools":to_value(vec![
            GetWeather::definition(),
            EditFile::definition(),
            ReadFile::definition(),
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

enum ToolsEnum {
    GetWeather(GetWeather),
    EditFile(EditFile),
    ReadFile(ReadFile),
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
    Ok(())
}

#[tauri::command]
async fn create_session(session_id: &str) -> Result<(), String> {
    let mut history_map = HISTORY_MAP.lock().await;
    history_map.entry(session_id.to_string()).or_insert_with(Vec::new);
    Ok(())
}

#[tauri::command]
async fn delete_session(session_id: &str) -> Result<(), String> {
    let mut history_map = HISTORY_MAP.lock().await;
    history_map.remove(session_id);
    Ok(())
}

#[tauri::command]
async fn call(session_id: &str, question: &str) -> Result<AgentReply, String> {
    let mut history = {
        let mut history_map = HISTORY_MAP.lock().await;
        let history = history_map
            .get_mut(session_id)
            .ok_or_else(|| "session不存在".to_string())?;
        history.push(InputMessage {
            role: Role::User,
            content: Some(question.trim_end().to_string()),
            reasoning_content: None,
            tool_call_id: None,
            tool_calls: None,
        });
        history.clone()
    };

    let tool_result_push =
        |history: &mut Vec<InputMessage>, res: anyhow::Result<String>, id: String| {
            let content = match res {
                Ok(value) => value,
                Err(err) => err.to_string(),
            };
            history.push(InputMessage {
                role: Role::Tool,
                content: Some(content),
                reasoning_content: None,
                tool_call_id: Some(id),
                tool_calls: None,
            });
        };
    loop {
        let res = agent_call(&history).await.map_err(|err| err.to_string())?;
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
                tool_call_id: None,
                tool_calls: Some(tool_calls.clone()),
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
                        tool_result_push(&mut history, result, tool.id.clone());
                    }
                    ("edit_file", Some(ToolsEnum::EditFile(edit_file))) => {
                        let result = serde_json::from_str::<EditFileInput>(&tool.function.arguments)
                            .map_err(|err| anyhow!(err))
                            .and_then(|input| edit_file.execute(input));
                        tool_result_push(&mut history, result, tool.id.clone());
                    }
                    ("read_file", Some(ToolsEnum::ReadFile(read_file))) => {
                        let result = serde_json::from_str::<ReadFileInput>(&tool.function.arguments)
                            .map_err(|err| anyhow!(err))
                            .and_then(|input| read_file.execute(input));
                        tool_result_push(&mut history, result, tool.id.clone());
                    }
                    _ => {
                        tool_result_push(
                            &mut history,
                            anyhow::Ok(format!("该{}工具未实现", &tool.function.name)),
                            tool.id.clone(),
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
            history.push(InputMessage {
                role: Role::Assistant,
                content: Some(response.clone()),
                reasoning_content: first_content_message.reasoning_content.clone(),
                tool_call_id: None,
                tool_calls: None,
            });
            let mut history_map = HISTORY_MAP.lock().await;
            history_map.insert(session_id.to_string(), history);
            return Ok(AgentReply {
                content: response.clone(),
                reasoning_content: first_content_message.reasoning_content.clone(),
            });
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
