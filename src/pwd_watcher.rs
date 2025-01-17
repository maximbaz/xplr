use crate::app::Task;
use crate::app::{ExternalMsg, MsgIn};
use anyhow::Result;
use notify::{watcher, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub fn keep_watching(
    pwd: &str,
    tx_msg_in: Sender<Task>,
    rx_pwd_watcher: Receiver<String>,
) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1))?;
    watcher.watch(pwd, RecursiveMode::NonRecursive)?;

    let mut last_pwd = pwd.to_string();
    thread::spawn(move || loop {
        if let Ok(new_pwd) = rx_pwd_watcher.try_recv() {
            watcher.unwatch(&last_pwd).unwrap();
            watcher
                .watch(&new_pwd, RecursiveMode::NonRecursive)
                .unwrap();
            last_pwd = new_pwd;
        } else {
            thread::sleep(Duration::from_secs(1));
        }

        if rx.try_recv().is_ok() {
            let msg = MsgIn::External(ExternalMsg::Explore);
            tx_msg_in.send(Task::new(3, msg, None)).unwrap();
        } else {
            thread::sleep(Duration::from_secs(1));
        }
    });
    Ok(())
}
