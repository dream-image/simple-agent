use std::path::PathBuf;
use crate::path::get_current_work_path;

pub fn get_system_prompt() ->String{
    let prompt =  format!("你是一个运行在simple-agent上的agent.\
     当前工作目录:{:?}", get_current_work_path());
    println!("{}", prompt);
    prompt
}