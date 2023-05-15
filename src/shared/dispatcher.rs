use std::error::Error;
use crate::shared::models::Task;
use crate::shared::msgbus::bus::Publisher;

pub struct Dispatcher<T: Publisher> {
    bus: T
}

impl<T: Publisher> Dispatcher<T> {
    pub fn new(bus: T) -> Self {
        Dispatcher { bus }
    }

    pub fn dispatch(&mut self, topic: String, task: Task) -> Result<(), Box<dyn Error>> {
        let msg = serde_json::to_string(&task)?;
        self.bus.publish(topic, msg)
    }
}