use std::error::Error;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::sync_channel;
use std::thread::spawn;
use futures::{StreamExt};
use futures::executor::block_on;
use crate::shared::models::Task;
use crate::shared::msgbus::bus::Consumer;

pub struct Executor<T: Consumer> {
    bus: T,
    topic: String,
    workers: usize,
}

impl<T: Consumer> Executor<T> {
    pub fn new(bus: T, cpus: usize, topic: String) -> Self {
        Executor {
            bus,
            topic,
            workers: cpus,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let (semaphore_rx, semaphore_tx) = sync_channel(self.workers);
        let semaphore_tx = Arc::new(Mutex::new(semaphore_tx));

        let mut msg_stream = self.bus.consume(self.topic.clone()).await?;
        while let Some(msg) = msg_stream.next().await {
            let task = serde_json::from_str::<Task>(&msg.body()).unwrap();
            if task.multithreaded {
                match exec(task.shell, task.command) {
                    Ok(_) => {
                        msg.ack().await.unwrap();
                    }
                    Err(err) => {
                        // TODO: unack message and requeue
                        return Err(err);
                    }
                }
            } else {
                let t = task.clone();
                let _ = semaphore_rx.send(());  // wait for available worker thread within limit
                let semaphore_tx = Arc::clone(&semaphore_tx);
                spawn(move || {
                    block_on(async {
                        match exec(t.shell, t.command) {
                            Ok(_) => {msg.ack().await.unwrap();}
                            Err(err) => {
                                // TODO: unack message and requeue
                                println!("Failed to execute task: {}", err);
                            }
                        }

                        let _ = semaphore_tx.lock().unwrap().recv();    // release worker thread
                    });
                });
            }
        }
        Ok(())
    }
}

fn exec(shell: String, cmd: String) -> Result<(), Box<dyn Error>> {
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