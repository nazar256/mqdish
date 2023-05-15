use std::error::Error;

pub trait Message: Sync {
    fn ack(&self) -> Result<(), Box<dyn Error>>;
    fn body(&self) -> String;
}

pub trait Publisher {
    fn publish(&mut self, topic: String, msg: String) -> Result<(), Box<dyn Error>>;
}

pub trait Consumer {
    fn consume(&mut self, topic: String, process: Box<dyn FnMut(Box<dyn Message + Send>)>) -> Result<(), Box<dyn Error>>;
}