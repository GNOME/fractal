use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use crate::util::client_url;
use crate::util::ResultExpectLog;

use crate::error::Error;

use crate::cache::CacheMap;

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
            access_token: String::new(),
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

    fn url(&self, base: Url, path: &str, mut params: Vec<(&str, String)>) -> Result<Url, Error> {
        let tk = self.data.lock().unwrap().access_token.clone();

        params.push(("access_token", tk));

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
                let r = register::login(self, user, passwd, &server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Logout(server)) => register::logout(self, server),
            Ok(BKCommand::Register(user, passwd, server)) => {
                let r = register::register(self, user, passwd, &server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Guest(server)) => {
                let r = register::guest(self, &server);
                bkerror!(r, tx, BKResponse::GuestLoginError);
            }
            Ok(BKCommand::SetToken(token, uid)) => register::set_token(self, token, uid),

            // User module
            Ok(BKCommand::GetUsername(server)) => user::get_username(self, server),
            Ok(BKCommand::SetUserName(server, name)) => user::set_username(self, server, name),
            Ok(BKCommand::GetThreePID(server)) => user::get_threepid(self, server),
            Ok(BKCommand::GetTokenEmail(server, identity, email, client_secret)) => {
                user::get_email_token(self, server, identity, email, client_secret)
            }
            Ok(BKCommand::GetTokenPhone(server, identity, phone, client_secret)) => {
                user::get_phone_token(self, server, identity, phone, client_secret)
            }
            Ok(BKCommand::SubmitPhoneToken(server, client_secret, sid, token)) => {
                user::submit_phone_token(self, server, client_secret, sid, token)
            }
            Ok(BKCommand::AddThreePID(server, identity, client_secret, sid)) => {
                user::add_threepid(self, server, identity, client_secret, sid)
            }
            Ok(BKCommand::DeleteThreePID(server, medium, address)) => {
                user::delete_three_pid(self, server, medium, address)
            }
            Ok(BKCommand::ChangePassword(server, username, old_password, new_password)) => {
                user::change_password(self, server, username, old_password, new_password)
            }
            Ok(BKCommand::AccountDestruction(server, username, password)) => {
                user::account_destruction(self, server, username, password)
            }
            Ok(BKCommand::GetAvatar(server)) => user::get_avatar(self, server),
            Ok(BKCommand::SetUserAvatar(server, file)) => user::set_user_avatar(self, server, file),
            Ok(BKCommand::GetAvatarAsync(server, member, ctx)) => {
                user::get_avatar_async(self, server, member, ctx)
            }
            Ok(BKCommand::GetUserInfoAsync(server, sender, ctx)) => {
                user::get_user_info_async(self, server, sender, ctx)
            }
            Ok(BKCommand::GetUserNameAsync(server, sender, ctx)) => {
                user::get_username_async(server, sender, ctx)
            }
            Ok(BKCommand::UserSearch(server, term)) => user::search(self, server, term),

            // Sync module
            Ok(BKCommand::Sync(server, since, initial)) => sync::sync(self, server, since, initial),
            Ok(BKCommand::SyncForced(server)) => sync::force_sync(self, server),

            // Room module
            Ok(BKCommand::GetRoomMembers(server, room)) => {
                let r = room::get_room_members(self, server, room);
                bkerror2!(r, tx, BKResponse::RoomMembers);
            }
            Ok(BKCommand::GetRoomMessages(server, room, from)) => {
                let r = room::get_room_messages(self, server, room, from);
                bkerror2!(r, tx, BKResponse::RoomMessagesTo);
            }
            Ok(BKCommand::GetRoomMessagesFromMsg(server, room, from)) => {
                room::get_room_messages_from_msg(self, server, room, from)
            }
            Ok(BKCommand::GetMessageContext(server, message)) => {
                let r = room::get_message_context(self, server, message);
                bkerror2!(r, tx, BKResponse::RoomMessagesTo);
            }
            Ok(BKCommand::SendMsg(server, msg)) => {
                let r = room::send_msg(self, server, msg);
                bkerror2!(r, tx, BKResponse::SentMsg);
            }
            Ok(BKCommand::SendMsgRedaction(server, msg)) => {
                let r = room::redact_msg(self, server, &msg);
                bkerror2!(r, tx, BKResponse::SentMsgRedaction);
            }
            Ok(BKCommand::SendTyping(server, room)) => {
                let r = room::send_typing(self, server, room);
                bkerror!(r, tx, BKResponse::SendTypingError);
            }
            Ok(BKCommand::SetRoom(server, id)) => {
                let r = room::set_room(self, server, id);
                bkerror!(r, tx, BKResponse::SetRoomError);
            }
            Ok(BKCommand::GetRoomAvatar(server, room)) => {
                let r = room::get_room_avatar(self, server, room);
                bkerror2!(r, tx, BKResponse::RoomAvatar);
            }
            Ok(BKCommand::JoinRoom(server, roomid)) => {
                let r = room::join_room(self, server, roomid);
                bkerror2!(r, tx, BKResponse::JoinRoom);
            }
            Ok(BKCommand::LeaveRoom(server, roomid)) => {
                let r = room::leave_room(self, server, &roomid);
                bkerror2!(r, tx, BKResponse::LeaveRoom);
            }
            Ok(BKCommand::MarkAsRead(server, roomid, evid)) => {
                let r = room::mark_as_read(self, server, &roomid, &evid);
                bkerror2!(r, tx, BKResponse::MarkedAsRead);
            }
            Ok(BKCommand::SetRoomName(server, roomid, name)) => {
                let r = room::set_room_name(self, server, &roomid, &name);
                bkerror2!(r, tx, BKResponse::SetRoomName);
            }
            Ok(BKCommand::SetRoomTopic(server, roomid, topic)) => {
                let r = room::set_room_topic(self, server, &roomid, &topic);
                bkerror2!(r, tx, BKResponse::SetRoomTopic);
            }
            Ok(BKCommand::SetRoomAvatar(server, roomid, fname)) => {
                let r = room::set_room_avatar(self, server, &roomid, &fname);
                bkerror2!(r, tx, BKResponse::SetRoomAvatar);
            }
            Ok(BKCommand::AttachFile(server, msg)) => {
                let r = room::attach_file(self, server, msg);
                bkerror2!(r, tx, BKResponse::AttachedFile);
            }
            Ok(BKCommand::NewRoom(server, name, privacy, internalid)) => {
                let r = room::new_room(self, server, &name, privacy, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internalid))
                        .expect_log("Connection closed");
                }
            }
            Ok(BKCommand::DirectChat(server, user, internalid)) => {
                let r = room::direct_chat(self, server, &user, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoom(Err(e), internalid))
                        .expect_log("Connection closed");
                }
            }
            Ok(BKCommand::AddToFav(server, roomid, tofav)) => {
                let r = room::add_to_fav(self, server, roomid, tofav);
                bkerror2!(r, tx, BKResponse::AddedToFav);
            }
            Ok(BKCommand::AcceptInv(server, roomid)) => {
                let r = room::join_room(self, server, roomid);
                bkerror2!(r, tx, BKResponse::JoinRoom);
            }
            Ok(BKCommand::RejectInv(server, roomid)) => {
                let r = room::leave_room(self, server, &roomid);
                bkerror2!(r, tx, BKResponse::LeaveRoom);
            }
            Ok(BKCommand::Invite(server, room, userid)) => {
                let r = room::invite(self, server, &room, &userid);
                bkerror!(r, tx, BKResponse::InviteError);
            }

            // Media module
            Ok(BKCommand::GetThumbAsync(server, media, ctx)) => {
                media::get_thumb_async(self, server, media, ctx)
            }
            Ok(BKCommand::GetMediaAsync(server, media, ctx)) => {
                media::get_media_async(self, server, media, ctx)
            }
            Ok(BKCommand::GetMediaListAsync(server, roomid, first_media_id, prev_batch, ctx)) => {
                media::get_media_list_async(self, server, &roomid, first_media_id, prev_batch, ctx)
            }
            Ok(BKCommand::GetMedia(server, media)) => media::get_media(self, server, media),
            Ok(BKCommand::GetMediaUrl(server, media, ctx)) => {
                media::get_media_url(self, server, media, ctx)
            }
            Ok(BKCommand::GetFileAsync(url, ctx)) => {
                let r = media::get_file_async(url, ctx);
                bkerror!(r, tx, BKResponse::GetFileAsyncError);
            }

            // Directory module
            Ok(BKCommand::DirectoryProtocols(server)) => directory::protocols(self, server),
            Ok(BKCommand::DirectorySearch(server, dhs, dq, dtp, more)) => {
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

                let r = directory::room_search(self, server, hs, q, tp, more);
                bkerror2!(r, tx, BKResponse::DirectorySearch);
            }

            // Stickers module
            Ok(BKCommand::ListStickers) => {
                let r = stickers::list(self);
                bkerror2!(r, tx, BKResponse::Stickers);
            }
            Ok(BKCommand::SendSticker(server, room, sticker)) => {
                let r = stickers::send(self, server, &room, &sticker);
                bkerror2!(r, tx, BKResponse::Stickers);
            }
            Ok(BKCommand::PurchaseSticker(group)) => {
                let r = stickers::purchase(self, &group);
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
