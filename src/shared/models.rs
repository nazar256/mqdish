use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub shell: String,
    pub command: String,
    pub multithreaded : bool,
}
