use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::util::ResultExpectLog;

use crate::cache::CacheMap;

use self::types::ThreadPool;

pub mod directory;
mod media;
pub mod register;
pub mod room;
mod sync;
mod types;
pub mod user;

pub use self::types::BKCommand;
pub use self::types::BKResponse;
pub use self::types::Backend;
pub use self::types::BackendData;
pub use self::types::RoomType;

impl Backend {
    pub fn new(tx: Sender<BKResponse>) -> Backend {
        let data = BackendData {
            rooms_since: String::new(),
            m_direct: HashMap::new(),
        };
        Backend {
            tx,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(60 * 60),
            thread_pool: ThreadPool::new(20),
        }
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

            // User module
            Ok(BKCommand::GetAvatarAsync(server, member, ctx)) => {
                user::get_avatar_async(self, server, member, ctx)
            }
            Ok(BKCommand::GetUserInfoAsync(server, sender, ctx)) => {
                user::get_user_info_async(self, server, sender, ctx)
            }

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
            Ok(BKCommand::SetRoom(server, access_token, room_id)) => {
                room::set_room(self, server, access_token, room_id)
            }
            Ok(BKCommand::AttachFile(server, access_token, msg)) => {
                let r = room::attach_file(self, server, access_token, msg);
                bkerror!(r, tx, BKResponse::AttachedFile);
            }
            Ok(BKCommand::DirectChat(server, access_token, uid, user, internal_id)) => {
                let data = self.data.clone();

                thread::spawn(move || {
                    let room_res = room::direct_chat(data, server, access_token, uid, user);
                    tx.send(BKResponse::NewRoom(room_res, internal_id))
                        .expect_log("Connection closed");
                });
            }

            // Media module
            Ok(BKCommand::GetThumbAsync(server, media, ctx)) => {
                media::get_thumb_async(self, server, media, ctx)
            }
            Ok(BKCommand::GetMediaAsync(server, media, ctx)) => {
                media::get_media_async(self, server, media, ctx)
            }
            Ok(BKCommand::GetMediaListAsync(
                server,
                access_token,
                room_id,
                first_media_id,
                prev_batch,
                ctx,
            )) => media::get_media_list_async(
                self,
                server,
                access_token,
                room_id,
                first_media_id,
                prev_batch,
                ctx,
            ),

            // Directory module
            Ok(BKCommand::DirectorySearch(server, access_token, dhs, dq, dtp, more)) => {
                let hs = match dhs {
                    ref a if a.is_empty() => None,
                    b => Some(b),
                };

                let q = match dq {
                    ref a if a.is_empty() => None,
                    b => Some(b),
                };

                let tp = match dtp {
                    ref a if a.is_empty() => None,
                    b => Some(b),
                };

                let r = directory::room_search(self, server, access_token, hs, q, tp, more);
                bkerror!(r, tx, BKResponse::DirectorySearch);
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
