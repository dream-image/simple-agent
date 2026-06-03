use std::cmp::max;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use crate::context::session_context::SessionContext;
use crate::tools::edit_file::{EditFile};
use crate::tools::get_weather::GetWeather;
use crate::tools::read_file::ReadFile;
use crate::tools::shell::Shell;
use crate::tools::{Tool, ToolEffect};

/**
`PermissionMod`:权限模式

| 模式                  | 直觉理解         | 适合的场景                    |
| ------------------- | ------------ | ------------------------ |
| `default`           | 默认安全模式       | 大多数日常使用                  |
| `acceptEdits`       | 自动接受文件编辑类动作  | 你明确想让 agent 改代码，但仍希望保留边界 |
| `plan`              | 只做计划，不做真实执行  | 先看方案、拆任务、做规划             |
| `dontAsk`           | 不再询问；没预授权就拒绝 | 无法交互确认、但又不想放开权限          |
| `bypassPermissions` | 基本跳过权限检查     | 非常信任环境，且明确接受高风险          |

 */
#[derive(Serialize, Clone, Debug,Deserialize,Default)]
pub enum PermissionMod{
    #[default]
    Default,
    AcceptEdits,
    Plan,
    DontAsk,
    BypassPermissions
}
#[derive(Serialize, Clone, Debug,Deserialize)]
pub struct ToolsPermission {
   pub deny_tools: HashSet<String>,
   pub ask_tools: HashSet<String>,
   pub allow_tools: HashSet<String>,
}

impl ToolsPermission {
    pub fn check_permissions(&self,tool_name:&str) ->PermissionLevel{
        if self.deny_tools.contains(tool_name){
            return PermissionLevel::Deny;
        };
        if self.ask_tools.contains(tool_name){
            return PermissionLevel::Ask;
        }
        PermissionLevel::Allow

    }
}

impl Default for ToolsPermission{
    fn default() -> Self {
        Self{
            deny_tools: HashSet::from([GetWeather::NAME.to_string()]),
            ask_tools: HashSet::from([EditFile::NAME.to_string(),Shell::NAME.to_string()]),
            allow_tools: HashSet::from([ReadFile::NAME.to_string()]),
        }
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq,Clone,Copy)]
pub enum PermissionLevel {
    Deny=3,
    Ask=2,
    Allow=1,
}

pub trait Permission:Tool {
    fn check_permission(&self,input:&Self::Input,context:&SessionContext) -> PermissionLevel;

}

pub fn check_final_permission<T:Tool>(pre_permission:&PermissionLevel, tool_permission:&PermissionLevel, permission_mod: &PermissionMod, tool:&T) ->(PermissionLevel,String){
    let name = T::NAME;
    let permission = *max(pre_permission,tool_permission);
    if permission == PermissionLevel::Deny{
        return (permission,"没有权限调用该工具".to_string());
    }
    match permission_mod {
        PermissionMod::Default => {
            return (permission,String::new());
        },
        PermissionMod::AcceptEdits => {
            if tool.effect_type(None) == ToolEffect::Write {
               return (PermissionLevel::Allow,String::new());
            }
             (permission,String::new())
        },
        PermissionMod::Plan => {
            if tool.effect_type(None) == ToolEffect::ReadOnly{
                return (PermissionLevel::Allow,String::new());
            }
            (PermissionLevel::Deny,"Plan模式只允许ReadOnly类工具使用".to_string())
        },
        PermissionMod::DontAsk => {
           if permission == PermissionLevel::Ask{
               return (PermissionLevel::Deny,"DontAsk模式下，ask被拒绝".to_string())
           }
            (permission,String::new())
        },
        PermissionMod::BypassPermissions => {
            if permission != PermissionLevel::Deny {
                return (PermissionLevel::Allow,String::new());
            }
            (permission,String::new())
        }

    }
}