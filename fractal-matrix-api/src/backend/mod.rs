use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use crate::util::client_url;

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
            server_url: Url::parse("https://matrix.org")
                .expect("Wrong server_url value in BackendData"),
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

    fn get_base_url(&self) -> Url {
        self.data.lock().unwrap().server_url.clone()
    }

    fn url(&self, path: &str, mut params: Vec<(&str, String)>) -> Result<Url, Error> {
        let base = self.get_base_url();
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
            Ok(BKCommand::Logout) => register::logout(self),
            Ok(BKCommand::Register(user, passwd, server)) => {
                let r = register::register(self, user, passwd, &server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Guest(server)) => {
                let r = register::guest(self, &server);
                bkerror!(r, tx, BKResponse::GuestLoginError);
            }
            Ok(BKCommand::SetToken(token, uid, server)) => {
                let r = register::set_token(self, token, uid, &server);
                bkerror!(r, tx, BKResponse::LoginError);
            }

            // User module
            Ok(BKCommand::GetUsername) => user::get_username(self),
            Ok(BKCommand::SetUserName(name)) => user::set_username(self, name),
            Ok(BKCommand::GetThreePID) => user::get_threepid(self),
            Ok(BKCommand::GetTokenEmail(identity, email, client_secret)) => {
                user::get_email_token(self, identity, email, client_secret)
            }
            Ok(BKCommand::GetTokenPhone(identity, phone, client_secret)) => {
                user::get_phone_token(self, identity, phone, client_secret)
            }
            Ok(BKCommand::SubmitPhoneToken(_, client_secret, sid, token)) => {
                user::submit_phone_token(self, client_secret, sid, token)
            }
            Ok(BKCommand::AddThreePID(identity, client_secret, sid)) => {
                user::add_threepid(self, identity, client_secret, sid)
            }
            Ok(BKCommand::DeleteThreePID(medium, address)) => {
                user::delete_three_pid(self, medium, address)
            }
            Ok(BKCommand::ChangePassword(username, old_password, new_password)) => {
                user::change_password(self, username, old_password, new_password)
            }
            Ok(BKCommand::AccountDestruction(username, password, _)) => {
                user::account_destruction(self, username, password)
            }
            Ok(BKCommand::GetAvatar) => user::get_avatar(self),
            Ok(BKCommand::SetUserAvatar(file)) => user::set_user_avatar(self, file),
            Ok(BKCommand::GetAvatarAsync(member, ctx)) => user::get_avatar_async(self, member, ctx),
            Ok(BKCommand::GetUserInfoAsync(sender, ctx)) => {
                user::get_user_info_async(self, &sender, ctx)
            }
            Ok(BKCommand::GetUserNameAsync(sender, ctx)) => {
                user::get_username_async(self, sender, ctx)
            }
            Ok(BKCommand::UserSearch(term)) => user::search(self, term),

            // Sync module
            Ok(BKCommand::Sync(since, initial)) => sync::sync(self, since, initial),
            Ok(BKCommand::SyncForced) => sync::force_sync(self),

            // Room module
            Ok(BKCommand::GetRoomMembers(room)) => {
                let r = room::get_room_members(self, room);
                bkerror!(r, tx, BKResponse::RoomMembersError);
            }
            Ok(BKCommand::GetRoomMessages(room, from)) => {
                let r = room::get_room_messages(self, room, from);
                bkerror!(r, tx, BKResponse::RoomMessagesError);
            }
            Ok(BKCommand::GetRoomMessagesFromMsg(room, from)) => {
                let r = room::get_room_messages_from_msg(self, room, from);
                bkerror!(r, tx, BKResponse::RoomMessagesError);
            }
            Ok(BKCommand::GetMessageContext(message)) => {
                let r = room::get_message_context(self, message);
                bkerror!(r, tx, BKResponse::RoomMessagesError);
            }
            Ok(BKCommand::SendMsg(msg)) => {
                let r = room::send_msg(self, msg);
                bkerror!(r, tx, BKResponse::SendMsgError);
            }
            Ok(BKCommand::SendMsgRedaction(msg)) => {
                let r = room::redact_msg(self, &msg);
                bkerror!(r, tx, BKResponse::SendMsgRedactionError);
            }
            Ok(BKCommand::SendTyping(room)) => {
                let r = room::send_typing(self, room);
                bkerror!(r, tx, BKResponse::SendTypingError);
            }
            Ok(BKCommand::SetRoom(id)) => {
                let r = room::set_room(self, id);
                bkerror!(r, tx, BKResponse::SetRoomError);
            }
            Ok(BKCommand::GetRoomAvatar(room)) => {
                let r = room::get_room_avatar(self, room);
                bkerror!(r, tx, BKResponse::GetRoomAvatarError);
            }
            Ok(BKCommand::JoinRoom(roomid)) => {
                let r = room::join_room(self, roomid);
                bkerror!(r, tx, BKResponse::JoinRoomError);
            }
            Ok(BKCommand::LeaveRoom(roomid)) => {
                let r = room::leave_room(self, &roomid);
                bkerror!(r, tx, BKResponse::LeaveRoomError);
            }
            Ok(BKCommand::MarkAsRead(roomid, evid)) => {
                let r = room::mark_as_read(self, &roomid, &evid);
                bkerror!(r, tx, BKResponse::MarkAsReadError);
            }
            Ok(BKCommand::SetRoomName(roomid, name)) => {
                let r = room::set_room_name(self, &roomid, &name);
                bkerror!(r, tx, BKResponse::SetRoomNameError);
            }
            Ok(BKCommand::SetRoomTopic(roomid, topic)) => {
                let r = room::set_room_topic(self, &roomid, &topic);
                bkerror!(r, tx, BKResponse::SetRoomTopicError);
            }
            Ok(BKCommand::SetRoomAvatar(roomid, fname)) => {
                let r = room::set_room_avatar(self, &roomid, &fname);
                bkerror!(r, tx, BKResponse::SetRoomAvatarError);
            }
            Ok(BKCommand::AttachFile(msg)) => {
                let r = room::attach_file(self, msg);
                bkerror!(r, tx, BKResponse::AttachFileError);
            }
            Ok(BKCommand::NewRoom(name, privacy, internalid)) => {
                let r = room::new_room(self, &name, privacy, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoomError(e, internalid)).unwrap();
                }
            }
            Ok(BKCommand::DirectChat(user, internalid)) => {
                let r = room::direct_chat(self, &user, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoomError(e, internalid)).unwrap();
                }
            }
            Ok(BKCommand::AddToFav(roomid, tofav)) => {
                let r = room::add_to_fav(self, roomid, tofav);
                bkerror!(r, tx, BKResponse::AddToFavError);
            }
            Ok(BKCommand::AcceptInv(roomid)) => {
                let r = room::join_room(self, roomid);
                bkerror!(r, tx, BKResponse::AcceptInvError);
            }
            Ok(BKCommand::RejectInv(roomid)) => {
                let r = room::leave_room(self, &roomid);
                bkerror!(r, tx, BKResponse::RejectInvError);
            }
            Ok(BKCommand::Invite(room, userid)) => {
                let r = room::invite(self, &room, &userid);
                bkerror!(r, tx, BKResponse::InviteError);
            }

            // Media module
            Ok(BKCommand::GetThumbAsync(media, ctx)) => {
                let r = media::get_thumb_async(self, media, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetMediaAsync(media, ctx)) => {
                let r = media::get_media_async(self, media, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetMediaListAsync(roomid, first_media_id, prev_batch, ctx)) => {
                let r = media::get_media_list_async(self, &roomid, first_media_id, prev_batch, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetMedia(media)) => {
                let r = media::get_media(self, media);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetMediaUrl(media, ctx)) => {
                let r = media::get_media_url(self, media.to_string(), ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetFileAsync(url, ctx)) => {
                let r = media::get_file_async(url, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }

            // Directory module
            Ok(BKCommand::DirectoryProtocols) => directory::protocols(self),
            Ok(BKCommand::DirectorySearch(dhs, dq, dtp, more)) => {
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

                let r = directory::room_search(self, hs, q, tp, more);
                bkerror!(r, tx, BKResponse::DirectoryError);
            }

            // Stickers module
            Ok(BKCommand::ListStickers) => {
                let r = stickers::list(self);
                bkerror!(r, tx, BKResponse::StickersError);
            }
            Ok(BKCommand::SendSticker(room, sticker)) => {
                let r = stickers::send(self, &room, &sticker);
                bkerror!(r, tx, BKResponse::StickersError);
            }
            Ok(BKCommand::PurchaseSticker(group)) => {
                let r = stickers::purchase(self, &group);
                bkerror!(r, tx, BKResponse::StickersError);
            }

            // Internal commands
            Ok(BKCommand::ShutDown) => {
                tx.send(BKResponse::ShutDown).unwrap();
                return false;
            }
            Err(_) => {
                return false;
            }
        };

        true
    }
}
