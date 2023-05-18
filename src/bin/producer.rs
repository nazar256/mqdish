use std::io::{BufRead, stdin};
use clap::Parser;
use futures::executor::block_on;
use mqdish::shared::config::{AppConfig, BusParams};
use mqdish::shared::dispatcher::Dispatcher;
use mqdish::shared::models::Task;
use mqdish::shared::msgbus::amqp::AmqpBus;

/// Distributes tasks as shell commands to be executed on multiple remote workers. It receives command to execute from stdin and publishes it to the chosen message broker.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // Topic name to publish task to.
    // Each topic should have consumers with same capabilities including resources.
    // So that tasks can be distributed among all workers within the same topic.
    // For workers with different capabilities (toolset or resources) you should use different topics.
    #[arg(short, long)]
    topic: Option<String>,

    // Shell to use for executing commands. Default is `sh`.
    #[arg(short, long)]
    shell: Option<String>,

    // How many tasks to run concurrently per CPU. Default is 1.
    // Sometimes due to nature of taks it can consume around half of CPU core,
    // in such case you can set concurrency_factor to 2.0 to run 2 tasks per CPU core.
    #[arg(short, long)]
    concurrency_factor: Option<u32>,

    // multithreaded task means that it scales over cpu cores itself.
    // If `true` only one such task can be run on a single node, `concurrency_factor` is ignored.
    #[arg(short, long)]
    multithreaded: Option<bool>,
}

fn main() {
    openssl_probe::init_ssl_cert_env_vars();
    let args = Args::parse();

    let config = AppConfig::load(None).expect("Failed to load config");
    let bus = block_on(async {
        match config.bus_params {
            BusParams::AMQP(_) => {
                AmqpBus::new(config.connection, config.credentials, config.bus_params)
                    .await
                    .expect("AMQP driver init failed")
            }
        }
    });
    let mut dispatcher = Dispatcher::new(bus);

    let shell = args.shell.unwrap_or("sh".to_string());
    let concurrency_factor = args.concurrency_factor.unwrap_or(1) as f32;
    let topic = match args.topic {
        Some(topic) => topic,
        None => {
            config.topic
        }
    };

    // Read input from stdin
    stdin()
        .lock()
        .lines()
        .filter_map(|line_result| match line_result {
            Ok(line) => Some(line),
            Err(error) => {
                eprintln!("Error reading line from STDIN: {}", error);
                None
            }
        })
        .for_each(|line| {
            let task = Task {
                shell: shell.clone(),
                command: line,
                concurrency_factor,
                multithreaded: args.multithreaded.unwrap_or_default(),
            };

            dispatcher.dispatch(topic.clone(), task).expect("Failed to dispatch task");
        });
}
