use crate::context::session_context::SessionContext;
use crate::permission::{Permission, PermissionLevel};
use crate::tools::{Tool, ToolEffect};
use anyhow::anyhow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
#[derive( Clone, Debug, Deserialize)]
pub struct Shell {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {

    #[schemars(description = "需要执行的shell命令，如果和shell_name共存，那么以此shell为准")]
    shell: Option<String>,
    #[schemars(description = "需要执行的shell命令名称")]
    shell_name:Option<String>,
    #[schemars(description = "shell_name对应的命令参数")]
    args: Option<Vec<String>>,

}
impl Tool for Shell {
    const NAME: &str = "shell";
    const DESCRIPTION: &'static str = "使用这个工具执行shell命令";
    type Input = ToolInput;
    type Output = anyhow::Result<String>;

    fn execute(&self, input: Self::Input,session_context: &SessionContext) -> Self::Output {
            let output = if let Some(shell) =input.shell {
                #[cfg(windows)]
                {
                    Command::new("cmd").current_dir(session_context.workspace.cwd.to_string()).args(["/C",ishell.as_str()]).output().map_err(|err| anyhow!(err))
                }
                #[cfg(not(windows))]
                {
                    Command::new("sh").current_dir(session_context.workspace.cwd.to_string()).args(["-c",shell.as_str()]).output().map_err(|err| anyhow!(err))
                }
            } else if let Some(shell_name)=input.shell_name {
                Command::new(shell_name).current_dir(session_context.workspace.cwd.to_string())
                    .args(input.args.as_ref().unwrap())
                    .output()
                    .map_err(|err| anyhow!(err))
            } else {
                Err(anyhow!("no shell name provided"))
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
