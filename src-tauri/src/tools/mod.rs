pub mod get_weather;
pub mod edit_file;
pub mod read_file;
pub mod shell;

use schemars::JsonSchema;
use serde_json::{Value, json, to_value};

pub trait Tool {
    //这里的&str等价于&'static str  只是 'static 被省略了
    const NAME: &str;
    const DESCRIPTION: &'static str;

    type Input: JsonSchema;
    type Output;
    fn execute(&self, input: Self::Input) -> Self::Output;

    fn definition() -> Value {
        let mut schema = to_value(schemars::schema_for!(Self::Input)).unwrap();
        if let Value::Object(map) = &mut schema {
            map.remove("title");
            map.remove("$schema");
        }
        json!({
           "type":"function",
            "function":{
                "name":Self::NAME,
                "description":Self::DESCRIPTION,
                "parameters":schema,
            }
        })
    }
}
