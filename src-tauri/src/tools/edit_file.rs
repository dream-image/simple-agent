use crate::tools::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{Error, Read, Write};

pub struct EditFile {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "文件的绝对路径，如果无法得出绝对路径地址，那就询问用户")]
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
        let mut content = fs::read_to_string(&input.file_path)?;

        if input.replace_all.unwrap_or(false) {
            content = content.replace(input.old_string.as_str(), &input.new_string);
        } else {
            content = content.replacen(input.old_string.as_str(), &input.new_string, 1);
        }
        fs::write(&input.file_path, content.as_bytes())?;
        Ok("修改成功".to_string())
    }
}
