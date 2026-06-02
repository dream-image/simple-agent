use crate::tools::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{Error, Read, Write};
use std::path::PathBuf;
use crate::path::getCurrentWorkPath;

pub struct EditFile {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "文件的路径,支持相对路径和绝对路径")]
    file_path: String,
    #[schemars(description = "被替换的字符串")]
    old_string: String,
    #[schemars(description = "替换的字符串")]
    new_string: String,
    #[schemars(
        description = "是否全量替换，默认是false，如果old_string多处匹配到，且replace_all是false，则失败",
    )]
    replace_all: Option<bool>,
}
impl Tool for EditFile {
    const NAME: &str = "edit_file";
    const DESCRIPTION: &'static str = "使用这个工具对文件进行编辑";
    type Input = ToolInput;
    type Output = anyhow::Result<String>;

    fn execute(&self, input: Self::Input) -> Self::Output {
        let mut path = PathBuf::from(input.file_path);
        if path.is_relative() {
            path = getCurrentWorkPath().join(path);
        }
        let mut content = fs::read_to_string(&path)?;

        if input.replace_all.unwrap_or(false) {
            content = content.replace(input.old_string.as_str(), &input.new_string);
        } else {
            content = content.replacen(input.old_string.as_str(), &input.new_string, 1);
        }
        fs::write(&path, content.as_bytes())?;
        Ok("修改成功".to_string())
    }
}
