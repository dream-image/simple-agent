use std::io::Write;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use tauri::{AppHandle, Manager};
use std::fs::{read_to_string, create_dir_all, write, exists, OpenOptions, remove_file};
use anyhow::anyhow;

pub static QUALIFIER: &str ="com";
pub static ORGANIZATION: &str ="ThousandDream";
pub static APPLICATION: &str ="simple-agent";

pub fn init_data_dir(app:&AppHandle)->anyhow::Result<()>{
     let dir = app.path().app_data_dir()?;
     println!("数据目录：{:?}", dir);
     // println!("库处理结果:{:?}",ProjectDirs::from(QUALIFIER,ORGANIZATION,APPLICATION).unwrap().data_dir().to_str().unwrap()); //这个值和上面的dir相等，所以后面处理直接用tauri提供的包就好
     std::fs::create_dir_all(&dir)?;
     Ok(())
}
pub fn write_file(app:&AppHandle,file_path: &str, contents: String)->anyhow::Result<bool>{
     let dir = app.path().app_data_dir()?;
     let file_path = dir.join(file_path);

     if exists(&file_path).is_ok() {
          write(&file_path, contents)?
     }else {
          create_dir_all(&file_path)?;
          write(&file_path, contents)?;
     };
     Ok(true)

}
pub fn add_file(app:&AppHandle,file_path: &str, contents: String)->anyhow::Result<bool>{
     let dir = app.path().app_data_dir()?;
     let file_path = dir.join(file_path);
     let mut file = OpenOptions::new().create(true).append(true).open(&file_path)?;
     writeln!(file, "{}", contents)?;
     Ok(true)
}
pub fn delete_file(app:&AppHandle,file_path: &str, contents: String)->anyhow::Result<bool>{
     let dir = app.path().app_data_dir()?;
     let file_path = dir.join(file_path);
     if exists(&file_path).is_ok() {
          remove_file(file_path)?;
     }else{
          return Err(anyhow!("文件不存在"));
     }
     Ok(true)
}

