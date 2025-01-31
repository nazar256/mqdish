# MqDiSh (Message Queue Distributed Shell)

MqDiSh is a distributed shell command execution system that allows you to publish shell tasks to multiple remote workers through a message queue broker. It consists of two main components:

- A producer CLI that publishes shell commands to a message queue
- Worker consumers that execute these commands across distributed nodes

## Features

- Distributed command execution across multiple workers
- Support for RabbitMQ as the message broker
- Configurable concurrency and worker distribution
- Support for both single-threaded and multi-threaded task execution
- YAML-based configuration
- Cross-platform support with statically linked binaries

## Installation

### Pre-built Binaries

Download the latest pre-built binaries from the [releases page](https://github.com/OWNER/mqdish/releases).

### Building from Source

1. Install Rust and Cargo
2. Clone the repository
3. Build the project:

```bash
cargo build --release
```

## Configuration

MqDiSh looks for configuration in the following locations (in order):
1. `/etc/mqdish/config.yaml`
2. `./mqdish.yaml`
3. `~/.config/mqdish/config.yaml`

Example configuration:

```yaml 
# credentials can be passed in AMQP connection string
connection: "amqp://user:pass@host:port/vhost?prefetch=4&heartbeat=60&consumer_timeout=300"
# credentials:
#   login: "user"
#   password: "pass"
bus_params:
  type: AMQP
  params:
    vhost: "/"
    prefetch: 4
    heartbeat: 60
    consumer_timeout: 300000 # consumer timeout in milliseconds
    requeue: false  # whether to requeue message after execution error, otherwise it will be dropped
topic: "mqdish" # topic to subscribe to, also used to dispatch commands if --topic is not specified
concurrency: 4 # number of commands to execute concurrently on each worker
```

## Usage

### Producer (Command Publisher)

Producer reqads STDIN and treats each line as a command to be executed by the consumer.
Producer exits once STDIN is closed.
This way you can dispatch batch of commands to the consumer in one line.
In more advanced way you can produce one command which will then be turned into batch on the consumer side.

```bash
mqdish --help
Distributes tasks as shell commands to be executed on multiple remote workers. It receives command to execute from stdin and publishes it to the chosen message broker

Usage: mqdish [OPTIONS]

Options:
  -t, --topic <TOPIC>                  
  -s, --shell <SHELL>                  
  -m, --exclusive <EXCLUSIVE>  [possible values: true, false]
  -h, --help                           Print help
  -V, --version                        Print version
```

- `-t, --topic <TOPIC>` - topic to publish commands to, if not specified, the topic from the configuration file will be used
- `-s, --shell <SHELL>` - shell to use for command execution, if not specified, the shell from the configuration file will be used
- `-m, --exclusive <EXCLUSIVE>` - whether to run this command exclusively on the worker.
When this flag is set to true, the worker will only receive next commands after the current one is finished
as if consumer concurrency was set to 1.

### Consumer (Worker)

Consumer does not have any options or arguments and configured only by the configuration file.

```bash
# Start a worker
mqdish-consumer

# The worker will automatically:
# - Connect to the configured message broker
# - Subscribe to the configured topic
# - Execute received commands
# - Handle concurrency based on configuration
```

## Docker Support

For the docker image of the worker refer to it's repository [here](https://github.com/nazar256/mqdish-workers-docker)



## Troubleshooting

### Consumer won't start

Consumer either hangs or returns the following message?

```sh
mqdish-consumer --help
thread 'main' panicked at src/bin/consumer.rs:15:18:
AMQP driver init failed: ConnectionFailure("err: IO error: connection aborted")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Check if you connect with `amqp://...` connection string to TSL secured broker.
In such case you need to use `amqps://...` connection string.