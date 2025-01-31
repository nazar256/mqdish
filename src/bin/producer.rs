use clap::Parser;
use mqdish::shared::config::{AppConfig, BusParams};
use mqdish::shared::dispatcher::Dispatcher;
use mqdish::shared::models::Task;
use mqdish::shared::msgbus::amqp::AmqpBus;
use mqdish::shared::msgbus::bus::Closer;
use openssl_probe::init_openssl_env_vars;
use std::io::{stdin, BufRead};

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

    // exclusive command means that it scales over cpu cores itself and must not be run concurrently.
    // If `true` only one such command should be run on a single node, `concurrency_factor` is ignored.
    #[arg(short, long)]
    exclusive: Option<bool>,
}

#[tokio::main]
async fn main() {
    unsafe {
        init_openssl_env_vars();
    }
    let args = Args::parse();

    let config = AppConfig::load(None).expect("Failed to load config");
    let mut bus = match config.bus_params {
        BusParams::AMQP(_) => {
            AmqpBus::new(config.connection, config.credentials, config.bus_params)
                .await
                .expect("AMQP driver init failed")
        }
    };

    let mut dispatcher = Dispatcher::new(&mut bus);

    let shell = args.shell.unwrap_or("sh".to_string());
    let topic = match args.topic {
        Some(topic) => topic,
        None => config.topic,
    };

    // TODO: maybe retry with backoff

    // Read input from stdin
    let tasks = stdin()
        .lock()
        .lines()
        .filter_map(|line_result| match line_result {
            Ok(line) => Some(line),
            Err(error) => {
                eprintln!("Error reading line from STDIN: {}", error);
                None
            }
        })
        .map(|line| async {
            Task {
                shell: shell.clone(),
                command: line,
                exclusive: args.exclusive.unwrap_or_default(),
            }
        });
    for task in tasks {
        dispatcher
            .dispatch(topic.clone(), task.await)
            .await
            .expect("Failed to dispatch task");
    }

    bus.close().await.expect("Failed to close bus");
}
