use std::error::Error;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::spawn;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use crate::shared::models::Task;
use crate::shared::msgbus::bus::Consumer;

pub struct Executor<'a, T: Consumer> {
    bus: &'a mut T,
    topic: String,
    workers: usize,
}

impl<'a, T: Consumer> Executor<'a, T> {
    pub fn new(bus: &'a mut T, cpus: usize, topic: String) -> Self {
        Executor {
            bus,
            topic,
            workers: cpus,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let (semaphore_rx, semaphore_tx) = channel(self.workers);
        let semaphore_tx = Arc::new(Mutex::new(semaphore_tx));

        let mut msg_stream = self.bus.consume(self.topic.clone()).await?;
        while let Some(msg) = msg_stream.next().await {
            let task = serde_json::from_str::<Task>(&msg.body())?;
            if task.multithreaded {
                match exec(task.shell, task.command) {
                    Ok(_) => {
                        msg.ack().await?;
                    }
                    Err(err) => {
                        // TODO: unack message and requeue
                        return Err(err);
                    }
                }
            } else {
                let t = task.clone();
                let semaphore_tx = Arc::clone(&semaphore_tx);
                let _ = semaphore_rx.send(());
                spawn(async move {
                    match exec(t.shell, t.command) {
                        Ok(_) => {
                            match msg.ack().await {
                                Ok(_) => {}
                                Err(err) => {
                                    println!("Failed to ack message: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            // TODO: unack message and requeue
                            println!("Failed to execute task: {}", err);
                        }
                    }

                    // release worker thread
                    match semaphore_tx.lock() {
                        Ok(mut tx) => {
                            let _ = tx.recv();
                        }
                        Err(err) => {
                            println!("Failed to release worker thread (recv from channel): {}", err);
                        }
                    }
                });
            }
        }
        Ok(())
    }
}

fn exec(shell: String, cmd: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut process = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = process.wait()?;
    if !status.success() {
        return Err(format!("Command exited with non-zero status: {}", status).into());
    }

    Ok(())
}