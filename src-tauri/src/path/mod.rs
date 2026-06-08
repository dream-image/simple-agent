use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use tauri::{AppHandle, Manager};
use crate::context::session_context::SessionContext;

pub static QUALIFIER: &str ="com";
pub static ORGANIZATION: &str ="ThousandDream";
pub static APPLICATION: &str ="simple-agent";

pub fn init_data_dir(app:&AppHandle)->anyhow::Result<()>{
     let dir = app.path().app_data_dir()?;
     println!("数据目录：{:?}", dir);
     std::fs::create_dir_all(&dir)?;
     Ok(())
}
// pub fn write_file(file_path: &str, contents: String)->anyhow::Result<bool>{
//      let data_dir=ProjectDirs::from(QUALIFIER,ORGANIZATION,APPLICATION);
//      let data_path = PathBuf::from()
// }
pub fn add_file(){

}
pub fn delete_file(){

}

