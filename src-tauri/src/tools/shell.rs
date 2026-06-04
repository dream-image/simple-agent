use crate::tools::{Tool, ToolEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
use anyhow::anyhow;
use crate::context::session_context::SessionContext;
use crate::permission::{Permission, PermissionLevel};

pub struct Shell {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "需要执行的shell名称")]
    shell: String,
    #[schemars(description = "具体参数数组")]
    args: Option<Vec<String>>,
}
impl Tool for Shell {
    const NAME: &str = "shell";
    const DESCRIPTION: &'static str = "使用这个工具执行shell命令";
    type Input = ToolInput;
    type Output = anyhow::Result<String>;

    fn execute(&self, input: Self::Input) -> Self::Output {
     let output= Command::new(input.shell).args(input.args.unwrap_or_default())
         .output()
         .map_err(|err| anyhow!(err));
        if let Err(err) = output{
            return Err(err);
        }
        let res = output.unwrap();
        if res.status.success() {
            Ok(String::from_utf8_lossy(&res.stdout).to_string())
        }else{
            Err(anyhow!(String::from_utf8_lossy(&res.stderr).to_string()))
        }
    }
    fn effect_type(&self,_:Option<&Self::Input>) -> ToolEffect {
        ToolEffect::Execute
    }
}

impl Permission for Shell {

    fn check_permission(&self,input:&Self::Input,session_context: &SessionContext) -> PermissionLevel {
        PermissionLevel::Ask
    }
}