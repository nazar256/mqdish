use std::{error};
use std::error::Error;
use std::pin::Pin;
use async_trait::async_trait;
use crate::shared::config::{BusParams, Credentials};
use crate::shared::config;
use crate::shared::msgbus::bus::{Closer, Consumer, Message, Publisher};
use thiserror::Error;
use lapin::{types::FieldTable, BasicProperties, ConnectionProperties, Channel, Connection};
use lapin::acker::Acker;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicQosOptions, ConfirmSelectOptions, QueueDeclareOptions, QueueDeleteOptions};
use tokio_stream::{Stream, StreamExt};
use crate::shared::config::Credentials::{LoginPassword, TLSClientAuth};

pub struct AmqpBus {
    #[allow(dead_code)] // we need to keep the connection alive
    connection: Connection,
    channel: Channel,
    consumption_queue: Option<String>,
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
    delivery_tag: Acker,
}

impl AmqpMessage {
    fn new(body: String, delivery_tag: Acker) -> Self {
        AmqpMessage {
            body,
            delivery_tag,
        }
    }
}

#[async_trait]
impl<'a> Message for AmqpMessage {
    async fn ack(&self) -> Result<(), Box<dyn Error>> {
        let _ = self.delivery_tag.ack(BasicAckOptions::default()).await;
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
        connection.on_error(|err| {
            panic!("Connection error: {:?}", err);
        });

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

        Ok(AmqpBus {
            connection,
            channel,
            consumption_queue: None,
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

#[async_trait]
impl Publisher for AmqpBus {
    async fn publish(&mut self, topic: String, msg: String) -> Result<(), Box<dyn error::Error>> {
        self.declare_queue(&topic).await?;

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
            // TODO: batch confirm
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
    }
}

#[async_trait]
impl Consumer for AmqpBus {
    async fn consume(&mut self, topic: String) -> Result<Pin<Box<dyn Stream<Item=Box<dyn Message + Send>>>>, Box<dyn Error>> {
        self.consumption_queue = Some(topic.clone());

        self.declare_queue(&topic).await?;

        let consumer = self.channel
            .basic_consume(
                topic.as_str(),
                "mqdish",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let msg_stream = consumer.filter_map(|delivery| {
            match delivery {
                Ok(delivery) => {
                    let body = String::from_utf8_lossy(delivery.data.as_slice()).to_string();
                    Some(Box::new(AmqpMessage::new(body, delivery.acker)) as Box<dyn Message + Send>)
                }
                _ => { None }
            }
        });

        Ok(Box::pin(msg_stream))
    }
}

#[async_trait]
impl Closer for AmqpBus {
    async fn close(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(queue) = &self.consumption_queue {
            let delete_opts = QueueDeleteOptions {
                if_empty: true,
                if_unused: true,
                nowait: false,
            };

            self.channel
                .queue_delete(queue, delete_opts)
                .await?;
        }

        Ok(())
    }
}