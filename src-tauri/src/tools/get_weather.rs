use crate::tools::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
}
