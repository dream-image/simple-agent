use crate::tools::{Tool, ToolEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::context::session_context::SessionContext;
use crate::permission::{Permission, PermissionLevel};

pub struct GetWeather {}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[schemars(title = "")]
pub struct ToolInput {
    #[schemars(description = "The city and state, e.g. San Francisco, CA")]
    city: String,
}
impl Tool for GetWeather {
    const NAME: &str = "get_weather";
    const DESCRIPTION: &'static str =
        "Get weather of a location, the user should supply a location first.";
    type Input = ToolInput;
    type Output = String;

    fn execute(&self, input: Self::Input) -> Self::Output {
        println!("工具入参：{:?}", input);
        format!("{} 温度是24度,无风", input.city)
    }
    fn effect_type(&self,_:Option<&Self::Input>) -> ToolEffect {
        ToolEffect::ReadOnly
    }
}

impl Permission for GetWeather {

    fn check_permission(&self,input:&Self::Input,session_context: &SessionContext) -> PermissionLevel {
        PermissionLevel::Allow
    }
}

