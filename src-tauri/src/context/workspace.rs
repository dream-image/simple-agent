use std::fmt::Display;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct Workspace{
    pub cwd:String,
    pub project_root:String,
    pub read_root:String,
    pub write_root:String,
}

impl Default for Workspace {
    fn default() -> Workspace {
        let current_dir =std::env::current_dir().unwrap().to_str().unwrap().to_string();
        Self{
            cwd:current_dir.clone(),
            project_root: current_dir.clone(),
            read_root: current_dir.clone(),
            write_root:current_dir.clone(),
        }
    }
}

impl Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(&self).unwrap())
    }
}




