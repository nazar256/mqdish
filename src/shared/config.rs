use std::fs::File;
use std::path::PathBuf;
use std::thread::available_parallelism;
use serde::Deserialize;
use serde_yaml::{self};
use thiserror::Error;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub connection: Connection,
    pub credentials: Credentials,
    pub bus_params: BusParams,
    pub topic: String,
    pub concurrency: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum BusParams {
    AMQP(AMQPParams),
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AMQPParams {
    pub vhost: String,
    pub prefetch: u16,
    pub heartbeat: Option<u16>,
    pub consumer_timeout: Option<i32>,
}

impl Default for BusParams {
    fn default() -> BusParams {
        BusParams::AMQP(AMQPParams::default())
    }
}

impl Default for AMQPParams {
    fn default() -> AMQPParams {
        AMQPParams {
            vhost: "/".to_string(),
            heartbeat: None,
            prefetch: available_parallelism().unwrap().get() as u16,
            consumer_timeout: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Connection {
    DSN(String),
    Params(ConnectionParams),
}

#[derive(Debug, Deserialize)]
pub struct ConnectionParams {
    pub host: String,
    pub port: u16,
    pub ssl: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Credentials {
    LoginPassword(LoginPassword),
    TLSClientAuth(TLSClientAuth),
    None,
}

#[derive(Debug, Deserialize)]
pub struct TLSClientAuth {
    pub ca_file: String,
    pub cert_file: String,
    pub key_file: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginPassword {
    pub login: String,
    pub password: String,
}

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            topic: "mqdish".to_string(),
            connection: Connection::DSN("amqp://".to_string()),
            credentials: Credentials::None,
            bus_params: BusParams::AMQP(AMQPParams::default()),
            concurrency: available_parallelism().unwrap().get(),
        }
    }
}


#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("No config file found in directories: {0}")]
    NotFound(String),
}

impl AppConfig {
    pub fn load(path: Option<String>) -> Result<AppConfig, ConfigError> {
        let paths = path.map_or_else(|| get_default_paths(), |path| vec![path]);

        let file = paths.iter().find_map(|path| File::open(path).ok());

        let config = match file {
            Some(file) => {
                let config: AppConfig = serde_yaml::from_reader(file).unwrap();
                config
            }
            _ => {
                return Err(ConfigError::NotFound(paths.join(", ")));
            }
        };

        Ok(config)
    }
}

fn get_default_paths() -> Vec<String> {
    let mut paths: Vec<String> = vec![
        "/etc/mqdish/config.yaml".to_string(),
        "./mqdish.yaml".to_string(),
    ];

    let home_dir: Option<PathBuf> = dirs::home_dir();
    match home_dir {
        Some(dir) => {
            let mut dir = dir;
            dir.push(".config");
            dir.push("mqdish");
            dir.push("config.yaml");
            paths.push(dir.to_str().unwrap_or_default().to_string());
        }
        _ => {}
    }

    paths
}