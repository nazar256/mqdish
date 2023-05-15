use std::thread::available_parallelism;
use mqdish::shared::config::{AppConfig, BusParams};
use mqdish::shared::executor::Executor;
use mqdish::shared::msgbus::amqp::AmqpBus;

fn main() {
    let config = AppConfig::load(None).expect("Failed to load config");
    let cpus = available_parallelism().unwrap().get();
    let bus = match config.bus_params {
        BusParams::AMQP(_) => {
            AmqpBus::new(config.connection, config.credentials, config.bus_params, cpus as u16)
                .expect("Failed to create AMQP bus")
        }
    };

    Executor::new(bus, cpus, config.topic).run().expect("Executor failed");
}