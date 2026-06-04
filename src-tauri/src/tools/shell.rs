use crate::context::session_context::SessionContext;
use crate::permission::{Permission, PermissionLevel};
use crate::tools::{Tool, ToolEffect};
use anyhow::anyhow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub struct Shell {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "需要执行的shell名称或者shell命令")]
    shell: String,
    #[schemars(description = "shell字段是名称的时候，需要传入具体参数")]
    args: Option<Vec<String>>,
}
impl Tool for Shell {
    const NAME: &str = "shell";
    const DESCRIPTION: &'static str = "使用这个工具执行shell命令";
    type Input = ToolInput;
    type Output = anyhow::Result<String>;

    fn execute(&self, input: Self::Input) -> Self::Output {

            let output = if (input.args.as_ref()).is_some_and(|x| x.len()>0){
                Command::new(input.shell)
                    .args(input.args.as_ref().unwrap())
                    .output()
                    .map_err(|err| anyhow!(err))
            }else{
                #[cfg(windows)]
                {
                    Command::new("cmd").args(["/C",input.shell.as_str()]).output().map_err(|err| anyhow!(err))
                }
                #[cfg(not(windows))]
                {
                    Command::new("sh").args(["-c",input.shell.as_str()]).output().map_err(|err| anyhow!(err))
                }
            };


            if let Err(err) = output {
                return Err(err);
            }
            let res = output.unwrap();
            if res.status.success() {
                Ok(String::from_utf8_lossy(&res.stdout).to_string())
            } else {
                Err(anyhow!(String::from_utf8_lossy(&res.stderr).to_string()))
            }

    }
    fn effect_type(&self, _: Option<&Self::Input>) -> ToolEffect {
        ToolEffect::Execute
    }
}

impl Permission for Shell {
    fn check_permission(
        &self,
        input: &Self::Input,
        session_context: &SessionContext,
    ) -> PermissionLevel {
        PermissionLevel::Ask
    }
}
