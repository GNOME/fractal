use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::util::ResultExpectLog;

pub mod directory;
pub mod media;
pub mod register;
pub mod room;
mod sync;
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
            // Register module
            Ok(BKCommand::Login(user, passwd, server, id_url)) => {
                register::login(self, user, passwd, server, id_url)
            }
            Ok(BKCommand::Register(user, passwd, server, id_url)) => {
                register::register(self, user, passwd, server, id_url)
            }
            Ok(BKCommand::Guest(server, id_url)) => register::guest(self, server, id_url),

            // Sync module
            Ok(BKCommand::Sync(server, access_token, uid, jtr, since, initial, number_tries)) => {
                sync::sync(
                    self,
                    server,
                    access_token,
                    uid,
                    jtr,
                    since,
                    initial,
                    number_tries,
                )
            }

            // Room module
            Ok(BKCommand::AttachFile(server, access_token, msg)) => {
                let r = room::attach_file(self, server, access_token, msg);
                bkerror!(r, tx, BKResponse::AttachedFile);
            }

            // Internal commands
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
