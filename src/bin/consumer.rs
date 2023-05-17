use std::thread::available_parallelism;
use futures::executor::block_on;
use mqdish::shared::config::{AppConfig, BusParams};
use mqdish::shared::executor::Executor;
use mqdish::shared::msgbus::amqp::AmqpBus;

fn main() {
    let config = AppConfig::load(None).expect("Failed to load config");
    let cpus = available_parallelism().unwrap().get();
    let bus = block_on(async {
        match config.bus_params {
            BusParams::AMQP(_) => {
                AmqpBus::new(config.connection, config.credentials, config.bus_params)
                    .await
                    .expect("AMQP driver init failed")
            }
        }
    });

    Executor::new(bus, cpus, config.topic).run().expect("Executor failed");
}