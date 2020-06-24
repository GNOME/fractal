use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::util::ResultExpectLog;

pub mod directory;
pub mod media;
pub mod register;
pub mod room;
pub mod sync;
mod types;
pub mod user;

pub use self::types::BKCommand;
pub use self::types::BKResponse;
pub use self::types::Backend;
pub use self::types::RoomType;
pub use self::types::ThreadPool;

impl Backend {
    pub fn new(tx: Sender<BKResponse>) -> Backend {
        Backend { tx }
    }

    pub fn run(mut self) -> Sender<BKCommand> {
        let (apptx, rx): (Sender<BKCommand>, Receiver<BKCommand>) = channel();

        thread::spawn(move || loop {
            let cmd = rx.recv();
            if !self.command_recv(cmd) {
                break;
            }
        });

        apptx
    }

    pub fn command_recv(&mut self, cmd: Result<BKCommand, RecvError>) -> bool {
        let tx = self.tx.clone();

        match cmd {
            Ok(BKCommand::SendBKResponse(response)) => {
                tx.send(response).expect_log("Connection closed");
            }
            Ok(BKCommand::ShutDown) => {
                tx.send(BKResponse::ShutDown)
                    .expect_log("Connection closed");
                return false;
            }
            Err(_) => {
                return false;
            }
        };

        true
    }
}
