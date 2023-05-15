use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub shell: String,
    pub command: String,
    pub concurrency_factor: f32,
    pub multithreaded : bool,
}
