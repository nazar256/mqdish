use std::{error};
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender};
use crate::shared::config::{BusParams, Connection, Credentials};
use crate::shared::config::Connection::*;
use crate::shared::msgbus::bus::{Consumer, Message, Publisher};
use amiquip::{AmqpProperties, Channel, ConsumerMessage, ConsumerOptions, Delivery, Publish, QueueDeclareOptions, QueueDeleteOptions};
use thiserror::Error;
use crate::shared::config::Credentials::{LoginPassword, TLSClientAuth};

pub struct AmqpBus {
    #[allow(dead_code)] // we need to keep the connection alive
    connection: amiquip::Connection,
    channel: Channel,
    consumption_queue: Option<String>,
    deliveries: HashMap<u64, Delivery>
}

#[derive(Error, Debug)]
pub enum AmqpError {
    #[error("Not implemented for AMQP driver: {0}")]
    NotImplemented(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Connection to AMQP server failed: {0}")]
    ConnectionFailure(String),
}

struct AmqpMessage {
    body: String,
    ack_notifier: Arc<Mutex<Sender<u64>>>,
    delivery_tag: u64,
}

impl AmqpMessage {
    fn new(body: String, ack_notifier: Sender<u64>, delivery_tag: u64) -> Self {
        AmqpMessage {
            body,
            ack_notifier: Arc::new(Mutex::new(ack_notifier)),
            delivery_tag,
        }
    }
}

impl<'a> Message for AmqpMessage {
    fn ack(&self) -> Result<(), Box<dyn Error>> {
        self.ack_notifier.lock().unwrap().send(self.delivery_tag)?;
        println!("ACK sent for delivery tag {}", self.delivery_tag);
        Ok(())
    }

    fn body(&self) -> String {
        self.body.clone()
    }
}

impl AmqpBus {
    pub fn new(connection_cfg: Connection, credentials: Credentials, bus_params: BusParams, prefetch: u16) -> Result<Self, AmqpError> {
        let auth_part = match credentials {
            LoginPassword(creds) => format!("{}:{}@", creds.login, creds.password),
            TLSClientAuth(_) => { return Err(AmqpError::NotImplemented("TLSClientAuth".to_string())); }
            Credentials::None => "".to_string(),
        };

        let amqp_params = match bus_params {
            BusParams::AMQP(params) => params,
        };

        let connection_url = match connection_cfg {
            DSN(dsn) => dsn,
            Params(params) => {
                format!(
                    "{}:{}//{}:{}/{}",
                    if params.ssl { "amqps" } else { "amqp" },
                    auth_part,
                    params.host,
                    params.port,
                    amqp_params.vhost,
                )
            }
        };
        let mut connection = match amiquip::Connection::insecure_open(&*connection_url) {
            Ok(conn) => conn,
            Err(e) => {
                let err = format!("{}, connection: {}", e.to_string(), connection_url);
                return Err(AmqpError::ConnectionFailure(err));
            }
        };

        let channel = match connection.open_channel(None) {
            Ok(channel) => { channel }
            Err(e) => {
                let err = format!("Failed to create AMQP channel: {}", e.to_string());
                return Err(AmqpError::ConnectionFailure(err));
            }
        };

        match channel.qos(0, prefetch+8, false) {
            Ok(_) => {}
            Err(e) => {
                let err = format!("Failed to set QoS: {}", e.to_string());
                return Err(AmqpError::ConnectionFailure(err));
            }
        }

        let deliveries = HashMap::new();

        Ok(AmqpBus { connection, channel, consumption_queue: None, deliveries })
    }
}

impl Publisher for AmqpBus {
    fn publish(&mut self, topic: String, msg: String) -> Result<(), Box<dyn error::Error>> {
        // declare queue before publish to make sure message will be stored in broker and delivered
        let queue_opts = QueueDeclareOptions {
            durable: true,  // queue should survive service restart
            ..QueueDeclareOptions::default()
        };
        self.channel.queue_declare(topic.clone(), queue_opts)?;

        let publish_props = AmqpProperties::default()
            .with_delivery_mode(2)
            .with_app_id("mqdish".to_string())
            .with_content_type("application/json".to_string());

        // Using default exchange binding
        self.channel.basic_publish("", Publish::with_properties(msg.as_bytes(), topic, publish_props))?;

        Ok(())
    }
}

impl Consumer for AmqpBus {
    fn consume(&mut self, topic: String, mut process: Box<dyn FnMut(Box<dyn Message + Send>)>) -> Result<(), Box<dyn Error>> {
        self.consumption_queue = Some(topic.clone());

        let queue_opts = QueueDeclareOptions {
            durable: true,  // queue should survive service restart
            ..QueueDeclareOptions::default()
        };
        let queue = self.channel.queue_declare(topic.clone(), queue_opts).unwrap();

        let consumer = queue.consume(ConsumerOptions::default()).unwrap();

        let (ack_rx, ack_tx) = channel();

        for msg in consumer.receiver().iter() {
            match msg {
                ConsumerMessage::Delivery(delivery) => {
                    delivery.delivery_tag();
                    let body = String::from_utf8_lossy(delivery.body.as_slice()).to_string();
                    let msg = AmqpMessage::new(body, ack_rx.clone(), delivery.delivery_tag());
                    println!("DELIVERY TAG: {} - job sent to processing", delivery.delivery_tag());
                    process(Box::new(msg));
                    println!("DELIVERY TAG: {} - job processed", delivery.delivery_tag());
                    self.deliveries.insert(delivery.delivery_tag(), delivery);

                    while let Ok(delivery_tag) = ack_tx.try_recv() {
                        println!("ACK received FROM CHANNEL for delivery tag {}", delivery_tag);
                        if let Some(delivery) = self.deliveries.remove(&delivery_tag) {
                            consumer.ack(delivery).expect("will acknowledge the message");
                        }

                    }
                }
                _ => {}
            }
        }

        // consumer.receiver().iter().for_each(async |msg| {
            // match msg {
            //     ConsumerMessage::Delivery(delivery) => {
            //         delivery.delivery_tag();
            //         let body = String::from_utf8_lossy(delivery.body.as_slice()).to_string();
            //         let msg = AmqpMessage::new(body, ack_rx.clone(), delivery.delivery_tag());
            //         process(Box::new(msg));
            //
            //         select! {
            //             Some(message) = ack_tx.recv() => {
            //                 // Process the message
            //                 println!("Received: {}", message);
            //             }
            //             _ = tokio::task::yield_now() => {
            //                 // No message received, do other processing or just continue the loop
            //                 println!("No message received");
            //             }
            //         }
            //     }
            //     _ => {}
            // }
        // });
        Ok(())
    }
}

impl Drop for AmqpBus {
    fn drop(&mut self) {
        if let Some(queue) = &self.consumption_queue {
            let delete_opts = QueueDeleteOptions {
                if_empty: true,
                if_unused: true,
            };
            self.channel
                .queue_delete(queue, delete_opts)
                .expect("Failed to delete unused queue");
        }
    }
}