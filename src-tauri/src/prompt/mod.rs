use std::path::PathBuf;
use crate::context::session_context::SessionContext;


pub fn get_system_prompt(session_context: Option<&SessionContext>) ->String{
    let session = if let Some(session) = session_context {
        session
    }else {
        &SessionContext::new(None,None)
    };
    let prompt =  format!("你是一个运行在simple-agent上的agent.\
     当前工作目录:{}", session.workspace);
    println!("{}", prompt);
    prompt
}