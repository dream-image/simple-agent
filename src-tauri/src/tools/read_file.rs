use crate::tools::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{Error, Read, Write};
use std::path::{Path, PathBuf};
use crate::path::getCurrentWorkPath;

pub struct ReadFile {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "文件的路径,支持相对路径和绝对路径")]
    file_path: String,
    #[schemars(description = "开始的行数，默认从0开始")]
    start_line: Option<usize>,
    #[schemars(description = "结束的行数，默认读到最后")]
    end_line: Option<usize>,
}
impl Tool for ReadFile {
    const NAME: &str = "read_file";
    const DESCRIPTION: &'static str = "使用这个工具对文件进行读取";
    type Input = ToolInput;
    type Output = anyhow::Result<String>;

    fn execute(&self, input: Self::Input) -> Self::Output {
        let mut path = PathBuf::from(input.file_path);
        if path.is_relative() {
            path = getCurrentWorkPath().join(path);
        }
        let mut content = fs::read_to_string(path)?;
        let start_line: usize = input.start_line.unwrap_or(0);
        let end_line: usize = input.end_line.unwrap_or(content.lines().count());
        content = content
            .split_inclusive('\n')
            .skip(start_line)
            .take(end_line.saturating_sub(start_line))
            .collect();
        Ok(content)
    }
}
