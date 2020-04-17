use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::util::dw_media;
use crate::util::ContentType;
use crate::util::ResultExpectLog;

use crate::cache::CacheMap;

use crate::globals;

use self::types::ThreadPool;

mod directory;
mod media;
pub mod register;
mod room;
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
            join_to_room: None,
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
            Ok(BKCommand::Sync(server, access_token, uid, since, initial)) => {
                sync::sync(self, server, access_token, uid, since, initial)
            }

            // Room module
            Ok(BKCommand::GetRoomMembers(server, access_token, room_id)) => {
                thread::spawn(move || {
                    let query = room::get_room_members(server, access_token, room_id);
                    tx.send(BKResponse::RoomMembers(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetRoomMessages(server, access_token, room_id, from)) => {
                thread::spawn(move || {
                    let query = room::get_room_messages(server, access_token, room_id, from);
                    tx.send(BKResponse::RoomMessagesTo(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetRoomMessagesFromMsg(server, access_token, room_id, from)) => {
                thread::spawn(move || {
                    let query =
                        room::get_room_messages_from_msg(server, access_token, room_id, from);
                    tx.send(BKResponse::RoomMessagesTo(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetMessageContext(server, access_token, message)) => {
                thread::spawn(move || {
                    let room_id = message.room.clone();
                    let event_id = &message.id;
                    let query = room::get_message_context(
                        server,
                        access_token,
                        room_id,
                        event_id,
                        globals::PAGE_LIMIT as u64,
                    );
                    tx.send(BKResponse::RoomMessagesTo(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SendMsg(server, access_token, msg)) => {
                thread::spawn(move || {
                    let query = room::send_msg(server, access_token, msg);
                    tx.send(BKResponse::SentMsg(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SendMsgRedaction(server, access_token, msg)) => {
                thread::spawn(move || {
                    let query = room::redact_msg(server, access_token, msg);
                    tx.send(BKResponse::SentMsgRedaction(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SendTyping(server, access_token, uid, room_id)) => {
                thread::spawn(move || {
                    let query = room::send_typing(server, access_token, uid, room_id);
                    if let Err(err) = query {
                        tx.send(BKResponse::SendTypingError(err))
                            .expect_log("Connection closed");
                    }
                });
            }
            Ok(BKCommand::SetRoom(server, access_token, room_id)) => {
                room::set_room(self, server, access_token, room_id)
            }
            Ok(BKCommand::GetRoomAvatar(server, access_token, room_id)) => {
                thread::spawn(move || {
                    let query = room::get_room_avatar(server, access_token, room_id);
                    tx.send(BKResponse::RoomAvatar(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::JoinRoom(server, access_token, room_id)) => {
                room::join_room(self, server, access_token, room_id)
            }
            Ok(BKCommand::LeaveRoom(server, access_token, room_id)) => {
                thread::spawn(move || {
                    let query = room::leave_room(server, access_token, room_id);
                    tx.send(BKResponse::LeaveRoom(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::MarkAsRead(server, access_token, room_id, evid)) => {
                thread::spawn(move || {
                    let query = room::mark_as_read(server, access_token, room_id, evid);
                    tx.send(BKResponse::MarkedAsRead(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SetRoomName(server, access_token, room_id, name)) => {
                thread::spawn(move || {
                    let query = room::set_room_name(server, access_token, room_id, name);
                    tx.send(BKResponse::SetRoomName(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SetRoomTopic(server, access_token, room_id, topic)) => {
                thread::spawn(move || {
                    let query = room::set_room_topic(server, access_token, room_id, topic);
                    tx.send(BKResponse::SetRoomTopic(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SetRoomAvatar(server, access_token, room_id, fname)) => {
                thread::spawn(move || {
                    let query = room::set_room_avatar(server, access_token, room_id, fname);
                    tx.send(BKResponse::SetRoomAvatar(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AttachFile(server, access_token, msg)) => {
                let r = room::attach_file(self, server, access_token, msg);
                bkerror!(r, tx, BKResponse::AttachedFile);
            }
            Ok(BKCommand::NewRoom(server, access_token, name, privacy, internal_id)) => {
                thread::spawn(move || {
                    let room_res = room::new_room(server, access_token, name, privacy);
                    tx.send(BKResponse::NewRoom(room_res, internal_id))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::DirectChat(server, access_token, uid, user, internal_id)) => {
                let data = self.data.clone();

                thread::spawn(move || {
                    let room_res = room::direct_chat(data, server, access_token, uid, user);
                    tx.send(BKResponse::NewRoom(room_res, internal_id))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AddToFav(server, access_token, uid, room_id, tofav)) => {
                thread::spawn(move || {
                    let query = room::add_to_fav(server, access_token, uid, room_id, tofav);
                    tx.send(BKResponse::AddedToFav(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AcceptInv(server, access_token, room_id)) => {
                room::join_room(self, server, access_token, room_id)
            }
            Ok(BKCommand::RejectInv(server, access_token, room_id)) => {
                thread::spawn(move || {
                    let query = room::leave_room(server, access_token, room_id);
                    tx.send(BKResponse::LeaveRoom(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::Invite(server, access_token, room_id, userid)) => {
                thread::spawn(move || {
                    let query = room::invite(server, access_token, room_id, userid);
                    if let Err(err) = query {
                        tx.send(BKResponse::InviteError(err))
                            .expect_log("Connection closed");
                    }
                });
            }
            Ok(BKCommand::ChangeLanguage(access_token, server, uid, room_id, lang)) => {
                thread::spawn(move || {
                    let query = room::set_language(access_token, server, uid, room_id, lang);
                    tx.send(BKResponse::ChangeLanguage(query))
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
            Ok(BKCommand::GetMedia(server, media)) => {
                thread::spawn(move || {
                    let fname = dw_media(server, &media, ContentType::Download, None);
                    tx.send(BKResponse::Media(fname))
                        .expect_log("Connection closed");
                });
            }

            // Directory module
            Ok(BKCommand::DirectoryProtocols(server, access_token)) => {
                thread::spawn(move || {
                    let query = directory::protocols(server, access_token);
                    tx.send(BKResponse::DirectoryProtocols(query))
                        .expect_log("Connection closed");
                });
            }
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
