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

mod directory;
mod media;
pub mod register;
mod room;
mod stickers;
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
            user_id: String::from("Guest"),
            scalar_token: None,
            scalar_url: Url::parse("https://scalar.vector.im")
                .expect("Wrong scalar_url value in BackendData"),
            sticker_widget: None,
            since: None,
            rooms_since: String::new(),
            join_to_room: String::new(),
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
            Ok(BKCommand::Login(user, passwd, server)) => {
                register::login(self, user, passwd, server)
            }
            Ok(BKCommand::Logout(server, access_token)) => {
                register::logout(self, server, access_token)
            }
            Ok(BKCommand::Register(user, passwd, server)) => {
                register::register(self, user, passwd, server)
            }
            Ok(BKCommand::Guest(server)) => register::guest(self, server),
            Ok(BKCommand::SetToken(token, uid)) => register::set_token(self, token, uid),

            // User module
            Ok(BKCommand::GetUsername(server)) => user::get_username(self, server),
            Ok(BKCommand::SetUserName(server, access_token, name)) => {
                user::set_username(self, server, access_token, name)
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
                username,
                old_password,
                new_password,
            )) => {
                thread::spawn(move || {
                    let query = user::change_password(
                        server,
                        access_token,
                        username,
                        old_password,
                        new_password,
                    );
                    tx.send(BKResponse::ChangePassword(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::AccountDestruction(server, access_token, username, password)) => {
                thread::spawn(move || {
                    let query = user::account_destruction(server, access_token, username, password);
                    tx.send(BKResponse::AccountDestruction(query))
                        .expect_log("Connection closed");
                });
            }
            Ok(BKCommand::GetAvatar(server)) => user::get_avatar(self, server),
            Ok(BKCommand::SetUserAvatar(server, access_token, file)) => {
                user::set_user_avatar(self, server, access_token, file)
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
            Ok(BKCommand::Sync(server, access_token, since, initial)) => {
                sync::sync(self, server, access_token, since, initial)
            }
            Ok(BKCommand::SyncForced(server, access_token)) => {
                sync::force_sync(self, server, access_token)
            }

            // Room module
            Ok(BKCommand::GetRoomMembers(server, access_token, room)) => {
                let r = room::get_room_members(self, server, access_token, room);
                bkerror2!(r, tx, BKResponse::RoomMembers);
            }
            Ok(BKCommand::GetRoomMessages(server, access_token, room, from)) => {
                let r = room::get_room_messages(self, server, access_token, room, from);
                bkerror2!(r, tx, BKResponse::RoomMessagesTo);
            }
            Ok(BKCommand::GetRoomMessagesFromMsg(server, access_token, room, from)) => {
                room::get_room_messages_from_msg(self, server, access_token, room, from)
            }
            Ok(BKCommand::GetMessageContext(server, access_token, message)) => {
                let r = room::get_message_context(self, server, access_token, message);
                bkerror2!(r, tx, BKResponse::RoomMessagesTo);
            }
            Ok(BKCommand::SendMsg(server, access_token, msg)) => {
                let r = room::send_msg(self, server, access_token, msg);
                bkerror2!(r, tx, BKResponse::SentMsg);
            }
            Ok(BKCommand::SendMsgRedaction(server, access_token, msg)) => {
                let r = room::redact_msg(self, server, access_token, &msg);
                bkerror2!(r, tx, BKResponse::SentMsgRedaction);
            }
            Ok(BKCommand::SendTyping(server, access_token, room)) => {
                let r = room::send_typing(self, server, access_token, room);
                bkerror!(r, tx, BKResponse::SendTypingError);
            }
            Ok(BKCommand::SetRoom(server, access_token, id)) => {
                let r = room::set_room(self, server, access_token, id);
                bkerror!(r, tx, BKResponse::SetRoomError);
            }
            Ok(BKCommand::GetRoomAvatar(server, access_token, room)) => {
                let r = room::get_room_avatar(self, server, access_token, room);
                bkerror2!(r, tx, BKResponse::RoomAvatar);
            }
            Ok(BKCommand::JoinRoom(server, access_token, roomid)) => {
                let r = room::join_room(self, server, access_token, roomid);
                bkerror2!(r, tx, BKResponse::JoinRoom);
            }
            Ok(BKCommand::LeaveRoom(server, access_token, roomid)) => {
                let r = room::leave_room(self, server, access_token, roomid);
                bkerror2!(r, tx, BKResponse::LeaveRoom);
            }
            Ok(BKCommand::MarkAsRead(server, access_token, roomid, evid)) => {
                let r = room::mark_as_read(self, server, access_token, roomid, evid);
                bkerror2!(r, tx, BKResponse::MarkedAsRead);
            }
            Ok(BKCommand::SetRoomName(server, access_token, roomid, name)) => {
                let r = room::set_room_name(self, server, access_token, roomid, name);
                bkerror2!(r, tx, BKResponse::SetRoomName);
            }
            Ok(BKCommand::SetRoomTopic(server, access_token, roomid, topic)) => {
                let r = room::set_room_topic(self, server, access_token, roomid, topic);
                bkerror2!(r, tx, BKResponse::SetRoomTopic);
            }
            Ok(BKCommand::SetRoomAvatar(server, access_token, roomid, fname)) => {
                let r = room::set_room_avatar(self, server, access_token, roomid, fname);
                bkerror2!(r, tx, BKResponse::SetRoomAvatar);
            }
            Ok(BKCommand::AttachFile(server, access_token, msg)) => {
                let r = room::attach_file(self, server, access_token, msg);
                bkerror2!(r, tx, BKResponse::AttachedFile);
            }
            Ok(BKCommand::NewRoom(server, access_token, name, privacy, internalid)) => {
                let r = room::new_room(
                    self,
                    server,
                    access_token,
                    name,
                    privacy,
                    internalid.clone(),
                );
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internalid))
                        .expect_log("Connection closed");
                }
            }
            Ok(BKCommand::DirectChat(server, access_token, user, internalid)) => {
                let r = room::direct_chat(self, server, access_token, user, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internalid))
                        .expect_log("Connection closed");
                }
            }
            Ok(BKCommand::AddToFav(server, access_token, roomid, tofav)) => {
                let r = room::add_to_fav(self, server, access_token, roomid, tofav);
                bkerror2!(r, tx, BKResponse::AddedToFav);
            }
            Ok(BKCommand::AcceptInv(server, access_token, roomid)) => {
                let r = room::join_room(self, server, access_token, roomid);
                bkerror2!(r, tx, BKResponse::JoinRoom);
            }
            Ok(BKCommand::RejectInv(server, access_token, roomid)) => {
                let r = room::leave_room(self, server, access_token, roomid);
                bkerror2!(r, tx, BKResponse::LeaveRoom);
            }
            Ok(BKCommand::Invite(server, access_token, room, userid)) => {
                let r = room::invite(self, server, access_token, room, userid);
                bkerror!(r, tx, BKResponse::InviteError);
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
                roomid,
                first_media_id,
                prev_batch,
                ctx,
            )) => media::get_media_list_async(
                self,
                server,
                access_token,
                roomid,
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
            Ok(BKCommand::GetMediaUrl(server, media, ctx)) => {
                media::get_media_url(self, server, media, ctx)
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

            // Stickers module
            Ok(BKCommand::ListStickers(access_token)) => {
                let r = stickers::list(self, access_token);
                bkerror2!(r, tx, BKResponse::Stickers);
            }
            Ok(BKCommand::SendSticker(server, access_token, room, sticker)) => {
                let r = stickers::send(self, server, access_token, room, sticker);
                bkerror2!(r, tx, BKResponse::Stickers);
            }
            Ok(BKCommand::PurchaseSticker(access_token, group)) => {
                let r = stickers::purchase(self, access_token, group);
                bkerror2!(r, tx, BKResponse::Stickers);
            }

            // Internal commands
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
