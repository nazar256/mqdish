use std::{error};
use std::error::Error;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use futures::executor::block_on;
use futures::StreamExt;
use crate::shared::config::{BusParams, Credentials};
use crate::shared::config;
use crate::shared::msgbus::bus::{Consumer, Message, Publisher};
use thiserror::Error;
use lapin::{types::FieldTable, BasicProperties, ConnectionProperties, Channel, Connection};
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicQosOptions, ConfirmSelectOptions, QueueDeclareOptions, QueueDeleteOptions};
use crate::shared::config::Credentials::{LoginPassword, TLSClientAuth};

pub struct AmqpBus {
    #[allow(dead_code)] // we need to keep the connection alive
    connection: Connection,
    // channel: Channel,
    channel: Channel,
    consumption_queue: Option<String>,
    ack_rx: Arc<Mutex<Sender<u64>>>,
    ack_tx: Receiver<u64>,
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
    fn new(body: String, ack_notifier: Arc<Mutex<Sender<u64>>>, delivery_tag: u64) -> Self {
        AmqpMessage {
            body,
            ack_notifier,
            delivery_tag,
        }
    }
}

impl<'a> Message for AmqpMessage {
    fn ack(&self) -> Result<(), Box<dyn Error>> {
        match self.ack_notifier.lock().unwrap().send(self.delivery_tag) {
            Ok(_) => {}
            Err(err) => {
                return Err(Box::new(AmqpError::ConnectionFailure(format!("Failed to send ACK notification: {}", err))));
            }
        }
        Ok(())
    }

    fn body(&self) -> String {
        self.body.clone()
    }
}

impl AmqpBus {
    pub async fn new(connection_cfg: config::Connection, credentials: Credentials, bus_params: BusParams) -> Result<Self, AmqpError> {
        let auth_part = match credentials {
            LoginPassword(creds) => format!("{}:{}@", creds.login, creds.password),
            TLSClientAuth(_) => { return Err(AmqpError::NotImplemented("TLSClientAuth".to_string())); }
            Credentials::None => "".to_string(),
        };

        let amqp_params = match bus_params {
            BusParams::AMQP(params) => params,
        };

        let connection_url = match connection_cfg {
            config::Connection::DSN(dsn) => dsn,
            config::Connection::Params(params) => {
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
        let conn_result = Connection::connect(&connection_url,
                                              ConnectionProperties::default(),
        ).await;

        let connection = match conn_result {
            Ok(connection) => { connection }
            Err(err) => { return Err(AmqpError::ConnectionFailure(format!("URL: {}, err: {}", connection_url, err))); }
        };

        let channel = match connection.create_channel().await {
            Ok(ch) => { ch }
            Err(err) => { return Err(AmqpError::ConnectionFailure(err.to_string())); }
        };
        match channel.basic_qos(amqp_params.prefetch, BasicQosOptions::default()).await {
            Ok(_) => {}
            Err(err) => { return Err(AmqpError::ConnectionFailure(err.to_string())); }
        }

        match channel.confirm_select(ConfirmSelectOptions::default()).await {
            Ok(_) => {}
            Err(err) => { return Err(AmqpError::ConnectionFailure(err.to_string())); }
        }

        let (ack_rx, ack_tx) = mpsc::channel();
        // let ack_tx = Arc::new(Mutex::new(ack_tx));
        let ack_rx = Arc::new(Mutex::new(ack_rx));

        Ok(AmqpBus {
            connection,
            channel,
            consumption_queue: None,
            ack_rx,
            ack_tx,
        })
    }

    async fn declare_queue(&mut self, topic: &String) -> Result<(), Box<dyn Error>> {
        let _ = self.channel
            .queue_declare(
                topic.as_str(),
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;
        Ok(())
    }
}

impl Publisher for AmqpBus {
    fn publish(&mut self, topic: String, msg: String) -> Result<(), Box<dyn error::Error>> {
        block_on( async {
            self.declare_queue(&topic).await.expect("queue declare");

            let msg_vec = msg.into_bytes();
            let publish = self.channel.basic_publish(
                "",
                topic.as_str(),
                BasicPublishOptions::default(),
                msg_vec.as_slice(),
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2)
                    .with_app_id("mqdish".into()),
            ).await;
            match publish {
                Err(err) => {
                    return Err(format!("Failed to publish message: {}", err).into());
                }
                Ok(confirm) => {
                    match confirm.await {
                        Ok(_) => {}
                        Err(err) => {
                            return Err(format!("Failed to publish message: {}", err).into());
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

impl Consumer for AmqpBus {
    fn consume(&mut self, topic: String, mut process: Box<dyn FnMut(Box<dyn Message + Send>)>) -> Result<(), Box<dyn Error>> {
        self.consumption_queue = Some(topic.clone());

        block_on(async {
            self.declare_queue(&topic).await.expect("queue declare");

            let mut consumer = self.channel
                .basic_consume(
                    topic.as_str(),
                    "mqdish",
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await.unwrap();

            let acker = async {
                for delivery_tag in &self.ack_tx {
                    self.channel.basic_ack(delivery_tag, BasicAckOptions::default()).await.expect("ack");
                }
            };

            while let Some(delivery) = consumer.next().await {
                let delivery = delivery.expect("no error in consumer");
                let body = String::from_utf8_lossy(delivery.data.as_slice()).to_string();
                let msg = AmqpMessage::new(body, Arc::clone(&self.ack_rx), delivery.delivery_tag);
                process(Box::new(msg));
                // write_deliveries.lock().unwrap().insert(delivery.delivery_tag(), delivery);
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .expect("ack");
            }

            acker.await;
        });

        Ok(())
    }
}

impl Drop for AmqpBus {
    fn drop(&mut self) {
        if let Some(queue) = &self.consumption_queue {
            let delete_opts = QueueDeleteOptions {
                if_empty: true,
                if_unused: true,
                nowait: true,
            };
            block_on(async {
                self.channel
                    .queue_delete(queue, delete_opts)
                    .await
                    .expect("Failed to delete unused queue");
            });
        }
    }
}