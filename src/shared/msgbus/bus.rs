use std::error::Error;
use std::pin::Pin;
use async_trait::async_trait;
use tokio_stream::Stream;

#[async_trait]
pub trait Message: Sync {
    async fn ack(&self) -> Result<(), Box<dyn Error>>;
    async fn nack(&self) -> Result<(), Box<dyn Error>>;
    fn body(&self) -> String;
}


#[async_trait]
pub trait Publisher {
    async fn publish(&mut self, topic: String, msg: String) -> Result<(), Box<dyn Error>>;
}

#[async_trait]
pub trait Consumer {
    async fn consume(&mut self, topic: String) -> Result<Pin<Box<dyn Stream<Item=Box<dyn Message + Send>>>>, Box<dyn Error>>;
}

#[async_trait]
pub trait Closer {
    async fn close(&mut self) -> Result<(), Box<dyn Error>>;
}