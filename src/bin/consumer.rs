use std::thread::available_parallelism;
use futures::executor::block_on;
use mqdish::shared::config::{AppConfig, BusParams};
use mqdish::shared::executor::Executor;
use mqdish::shared::msgbus::amqp::AmqpBus;

fn main() {
    openssl_probe::init_ssl_cert_env_vars();
    let config = AppConfig::load(None).expect("Failed to load config");
    let cpus = available_parallelism().unwrap().get();  // TODO: maybe make configurable
    block_on(async {
        let bus = match config.bus_params {
            BusParams::AMQP(_) => {
                AmqpBus::new(config.connection, config.credentials, config.bus_params)
                    .await
                    .expect("AMQP driver init failed")
            }
        };

        Executor::new(bus, cpus, config.topic).run().await.expect("Executor failed");
    });
}