use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use crate::util::client_url;
use crate::util::dw_media;
use crate::util::ContentType;
use crate::util::ResultExpectLog;

use crate::error::Error;

use crate::cache::CacheMap;

use crate::r0::AccessToken;

use crate::globals;

mod directory;
mod media;
pub mod register;
mod room;
mod sync;
mod types;
mod user;

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
            internal_tx: None,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(60 * 60),
            limit_threads: Arc::new((Mutex::new(0u8), Condvar::new())),
        }
    }

    fn url(
        &self,
        base: Url,
        tk: &AccessToken,
        path: &str,
        mut params: Vec<(&str, String)>,
    ) -> Result<Url, Error> {
        params.push(("access_token", tk.to_string()));

        client_url(&base, path, &params)
    }

    pub fn run(mut self) -> Sender<BKCommand> {
        let (apptx, rx): (Sender<BKCommand>, Receiver<BKCommand>) = channel();

        self.internal_tx = Some(apptx.clone());
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
            Ok(BKCommand::Logout(server, access_token)) => {
                thread::spawn(move || {
                    let query = register::logout(server, access_token);
                    tx.send(BKResponse::Logout(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::Register(user, passwd, server, id_url)) => {
                register::register(self, user, passwd, server, id_url)
            }
            Ok(BKCommand::Guest(server, id_url)) => register::guest(self, server, id_url),

            // User module
            Ok(BKCommand::GetUsername(server, uid)) => {
                thread::spawn(move || {
                    let query = user::get_username(server, uid);
                    tx.send(BKResponse::Name(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SetUserName(server, access_token, uid, username)) => {
                thread::spawn(move || {
                    let query = user::set_username(server, access_token, uid, username);
                    tx.send(BKResponse::SetUserName(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetThreePID(server, access_token)) => {
                thread::spawn(move || {
                    let query = user::get_threepid(server, access_token);
                    tx.send(BKResponse::GetThreePID(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetTokenEmail(server, access_token, identity, email, client_secret)) => {
                thread::spawn(move || {
                    let query =
                        user::get_email_token(server, access_token, identity, email, client_secret);
                    tx.send(BKResponse::GetTokenEmail(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetTokenPhone(server, access_token, identity, phone, client_secret)) => {
                thread::spawn(move || {
                    let query =
                        user::get_phone_token(server, access_token, identity, phone, client_secret);
                    tx.send(BKResponse::GetTokenPhone(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SubmitPhoneToken(server, client_secret, sid, token)) => {
                thread::spawn(move || {
                    let query = user::submit_phone_token(server, client_secret, sid, token);
                    tx.send(BKResponse::SubmitPhoneToken(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AddThreePID(server, access_token, identity, client_secret, sid)) => {
                thread::spawn(move || {
                    let query =
                        user::add_threepid(server, access_token, identity, client_secret, sid);
                    tx.send(BKResponse::AddThreePID(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::DeleteThreePID(server, access_token, medium, address)) => {
                thread::spawn(move || {
                    let query = user::delete_three_pid(server, access_token, medium, address);
                    tx.send(BKResponse::DeleteThreePID(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::ChangePassword(
                server,
                access_token,
                user,
                old_password,
                new_password,
            )) => {
                thread::spawn(move || {
                    let query = user::change_password(
                        server,
                        access_token,
                        user,
                        old_password,
                        new_password,
                    );
                    tx.send(BKResponse::ChangePassword(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AccountDestruction(server, access_token, user, password)) => {
                thread::spawn(move || {
                    let query = user::account_destruction(server, access_token, user, password);
                    tx.send(BKResponse::AccountDestruction(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetAvatar(server, uid)) => {
                thread::spawn(move || {
                    let query = user::get_avatar(server, uid);
                    tx.send(BKResponse::Avatar(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::SetUserAvatar(server, access_token, uid, file)) => {
                thread::spawn(move || {
                    let query = user::set_user_avatar(server, access_token, uid, file);
                    tx.send(BKResponse::SetUserAvatar(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetAvatarAsync(server, member, ctx)) => {
                user::get_avatar_async(self, server, member, ctx)
            }
            Ok(BKCommand::GetUserInfoAsync(server, sender, ctx)) => {
                user::get_user_info_async(self, server, sender, ctx)
            }
            Ok(BKCommand::GetUserNameAsync(server, sender, ctx)) => {
                thread::spawn(move || {
                    let query = user::get_username_async(server, sender);
                    ctx.send(query).expect_log("Connection closed");
                });
            }
            Ok(BKCommand::UserSearch(server, access_token, term)) => {
                thread::spawn(move || {
                    let query = user::search(server, access_token, term);
                    tx.send(BKResponse::UserSearch(query))
                        .expect_log("Connection closed");
                });
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
                room::get_room_messages_from_msg(self, server, access_token, room_id, from)
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
                let r = room::set_room_topic(self, server, access_token, room_id, topic);
                bkerror2!(r, tx, BKResponse::SetRoomTopic);
            }
            Ok(BKCommand::SetRoomAvatar(server, access_token, room_id, fname)) => {
                let r = room::set_room_avatar(self, server, access_token, room_id, fname);
                bkerror2!(r, tx, BKResponse::SetRoomAvatar);
            }
            Ok(BKCommand::AttachFile(server, access_token, msg)) => {
                let r = room::attach_file(self, server, access_token, msg);
                bkerror2!(r, tx, BKResponse::AttachedFile);
            }
            Ok(BKCommand::NewRoom(server, access_token, name, privacy, internal_id)) => {
                let r = room::new_room(
                    self,
                    server,
                    access_token,
                    name,
                    privacy,
                    internal_id.clone(),
                );
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internal_id))
                        .expect_log("Connection closed");
                }
            }
            Ok(BKCommand::DirectChat(server, access_token, uid, user, internal_id)) => {
                let r =
                    room::direct_chat(self, server, access_token, uid, user, internal_id.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internal_id))
                        .expect_log("Connection closed");
                }
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
                let r = room::set_language(self, access_token, server, uid, room_id, lang);
                bkerror2!(r, tx, BKResponse::ChangeLanguage);
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
                    let fname = dw_media(&server, &media, ContentType::Download, None);
                    tx.send(BKResponse::Media(fname))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetFileAsync(url, ctx)) => {
                let r = media::get_file_async(url, ctx);
                bkerror!(r, tx, BKResponse::GetFileAsyncError);
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
                bkerror2!(r, tx, BKResponse::DirectorySearch);
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
