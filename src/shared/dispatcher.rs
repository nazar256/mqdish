use crate::shared::models::Task;
use crate::shared::msgbus::bus::Publisher;
use std::error::Error;

pub struct Dispatcher<'a, T: Publisher> {
    bus: &'a mut T,
}

impl<'a, T: Publisher> Dispatcher<'a, T> {
    pub fn new(bus: &'a mut T) -> Self {
        Dispatcher { bus }
    }

    pub async fn dispatch(&mut self, topic: String, task: Task) -> Result<(), Box<dyn Error>> {
        let msg = serde_json::to_string(&task)?;
        self.bus.publish(topic, msg).await
    }
}
