use std::error::Error;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::sync_channel;
use std::thread::spawn;
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

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let (semaphore_rx, semaphore_tx) = sync_channel(self.workers);
        let semaphore_tx = Arc::new(Mutex::new(semaphore_tx));

        self.bus.consume(self.topic.clone(), Box::new( move |msg| {
            let task = serde_json::from_str::<Task>(&msg.body()).unwrap();
            if task.multithreaded {
                exec(task.shell, task.command);
                msg.ack().unwrap();
            } else {
                let t = task.clone();
                let _ = semaphore_rx.send(());  // wait for available worker thread within limit
                let semaphore_tx = Arc::clone(&semaphore_tx);
                spawn(move || {
                    exec(t.shell, t.command);
                    msg.ack().unwrap();
                    let _ = semaphore_tx.lock().unwrap().recv();    // release worker thread
                });
            }
        }))
    }
}

fn exec(shell: String, cmd: String) {
    let mut process = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn().expect("Failed to execute command");

    let res = process.wait();

    match res {
        Ok(status) => {
            if !status.success() {
                eprintln!("Command exited with non-zero status: {}", status)
            }
        }
        Err(e) => {
            eprintln!("error executing command: {}", e)
        }
    }
}