use crate::shared::models::Task;
use crate::shared::msgbus::bus::Consumer;
use std::error::Error;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::spawn;
use tokio::sync::{mpsc::channel, Mutex};
use tokio_stream::StreamExt;

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
        let (semaphore_tx, semaphore_rx) = channel(self.workers);
        // let semaphore_rx = Arc::new(Mutex::new(semaphore_rx));
        let semaphore_rx = Arc::new(Mutex::new(semaphore_rx));
        // semaphore_rx.

        let mut msg_stream = self.bus.consume(self.topic.clone()).await?;
        while let Some(msg) = msg_stream.next().await {
            let task = serde_json::from_str::<Task>(&msg.body())?;
            if task.exclusive {
                match exec(task.shell, task.command).await {
                    Ok(_) => {
                        msg.ack().await?;
                    }
                    Err(err) => {
                        msg.nack().await?;
                        println!("Failed to execute task: {}", err);
                    }
                }
            } else {
                let t = task.clone();
                let semaphore_rx = Arc::clone(&semaphore_rx);
                let _ = semaphore_tx.send(()).await;
                // TODO: properly handle errors inside future
                spawn(async move {
                    match exec(t.shell, t.command).await {
                        Ok(_) => match msg.ack().await {
                            Ok(_) => {}
                            Err(err) => {
                                println!("Failed to ack message: {}", err);
                            }
                        },
                        Err(err) => {
                            let _ = msg.nack().await;
                            println!("Failed to execute task: {}", err);
                        }
                    }

                    semaphore_rx.lock().await.recv().await;
                });
            }
        }
        Ok(())
    }
}

async fn exec(shell: String, cmd: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut process = Command::new(shell)
        .arg("-c")
        .arg(cmd.clone())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = process.wait().await?;
    if !status.success() {
        return Err(format!(
            "Command exited with non-zero status: {}, command: {}",
            status, cmd
        )
        .into());
    }

    Ok(())
}
