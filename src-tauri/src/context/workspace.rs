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
        Self{
            cwd: "/".to_string(),
            project_root: "/".to_string(),
            read_root: "/".to_string(),
            write_root: "/".to_string(),
        }
    }
}




