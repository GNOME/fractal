use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use util::client_url;

use error::Error;

use cache::CacheMap;

mod directory;
mod media;
mod register;
mod room;
mod stickers;
mod sync;
mod types;
mod user;

pub use self::types::BKCommand;
pub use self::types::BKResponse;

use self::types::BKCommand::*;

pub use self::types::Backend;
pub use self::types::BackendData;

pub use self::types::RoomType;

impl Backend {
    pub fn new(tx: Sender<BKResponse>) -> Self {
        let data = BackendData {
            user_id: "Guest".to_string(),
            access_token: String::new(),
            server_url: "https://matrix.org".to_string(),
            scalar_token: None,
            scalar_url: "https://scalar.vector.im".to_string(),
            sticker_widget: None,
            since: None,
            rooms_since: String::new(),
            join_to_room: String::new(),
            m_direct: HashMap::new(),
        };

        Self {
            tx: tx,
            internal_tx: None,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(60 * 60),
            limit_threads: Arc::new((Mutex::new(0u8), Condvar::new())),
        }
    }

    fn get_base_url(&self) -> Result<Url, Error> {
        let s = self.data.lock().unwrap().server_url.clone();
        Ok(Url::parse(&s)?)
    }

    fn url(&self, path: &str, mut params: Vec<(&str, String)>) -> Result<Url, Error> {
        let base = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();

        params.push(("access_token", tk));

        client_url(&base, path, params)
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

        cmd.map(|cmd| {
            match cmd {
                // Register module
                Login(user, passwd, server) => {
                    let r = register::login(self, user, passwd, server);
                    bkerror!(r, tx, BKResponse::LoginError);
                }
                Logout => {
                    let r = register::logout(self);
                    bkerror!(r, tx, BKResponse::LogoutError);
                }
                Register(user, passwd, server) => {
                    let r = register::register(self, user, passwd, server);
                    bkerror!(r, tx, BKResponse::LoginError);
                }
                Guest(server) => {
                    let r = register::guest(self, server);
                    bkerror!(r, tx, BKResponse::GuestLoginError);
                }
                SetToken(token, uid, server) => {
                    let r = register::set_token(self, token, uid, server);
                    bkerror!(r, tx, BKResponse::LoginError);
                }
                // User module
                GetUsername => {
                    let r = user::get_username(self);
                    bkerror!(r, tx, BKResponse::UserNameError);
                }
                SetUserName(name) => {
                    let r = user::set_username(self, name);
                    bkerror!(r, tx, BKResponse::SetUserNameError);
                }
                GetThreePID => {
                    let r = user::get_threepid(self);
                    bkerror!(r, tx, BKResponse::GetThreePIDError);
                }
                GetTokenEmail(identity, email, client_secret) => {
                    let r = user::get_email_token(self, identity, email, client_secret);
                    bkerror!(r, tx, BKResponse::GetTokenEmailError);
                }
                GetTokenPhone(identity, phone, client_secret) => {
                    let r = user::get_phone_token(self, identity, phone, client_secret);
                    bkerror!(r, tx, BKResponse::GetTokenEmailError);
                }
                SubmitPhoneToken(identity, client_secret, sid, token) => {
                    let r = user::submit_phone_token(self, identity, client_secret, sid, token);
                    bkerror!(r, tx, BKResponse::SubmitPhoneTokenError);
                }
                AddThreePID(identity, client_secret, sid) => {
                    let r = user::add_threepid(self, identity, client_secret, sid);
                    bkerror!(r, tx, BKResponse::AddThreePIDError);
                }
                DeleteThreePID(medium, address) => {
                    let r = user::delete_three_pid(self, medium, address);
                    bkerror!(r, tx, BKResponse::DeleteThreePIDError);
                }
                ChangePassword(username, old_password, new_password) => {
                    let r = user::change_password(self, username, old_password, new_password);
                    bkerror!(r, tx, BKResponse::ChangePasswordError);
                }
                AccountDestruction(username, password, flag) => {
                    let r = user::account_destruction(self, username, password, flag);
                    bkerror!(r, tx, BKResponse::AccountDestructionError);
                }
                GetAvatar => {
                    let r = user::get_avatar(self);
                    bkerror!(r, tx, BKResponse::AvatarError);
                }
                SetUserAvatar(file) => {
                    let r = user::set_user_avatar(self, file);
                    bkerror!(r, tx, BKResponse::SetUserAvatarError);
                }
                GetAvatarAsync(member, ctx) => {
                    let r = user::get_avatar_async(self, member, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetUserInfoAsync(sender, ctx) => {
                    let r = user::get_user_info_async(self, &sender, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetUserNameAsync(sender, ctx) => {
                    let r = user::get_username_async(self, sender, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                UserSearch(term) => {
                    let r = user::search(self, term);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                // Sync module
                Sync(since, initial) => {
                    let r = sync::sync(self, since, initial);
                    bkerror!(r, tx, BKResponse::SyncError);
                }
                SyncForced => {
                    let r = sync::force_sync(self);
                    bkerror!(r, tx, BKResponse::SyncError);
                }
                // Room module
                GetRoomMembers(room) => {
                    let r = room::get_room_members(self, room);
                    bkerror!(r, tx, BKResponse::RoomMembersError);
                }
                GetRoomMessages(room, from) => {
                    let r = room::get_room_messages(self, room, from);
                    bkerror!(r, tx, BKResponse::RoomMessagesError);
                }
                GetRoomMessagesFromMsg(room, from) => {
                    let r = room::get_room_messages_from_msg(self, room, from);
                    bkerror!(r, tx, BKResponse::RoomMessagesError);
                }
                GetMessageContext(message) => {
                    let r = room::get_message_context(self, message);
                    bkerror!(r, tx, BKResponse::RoomMessagesError);
                }
                SendMsg(msg) => {
                    let r = room::send_msg(self, msg);
                    bkerror!(r, tx, BKResponse::SendMsgError);
                }
                SendMsgRedaction(msg) => {
                    let r = room::redact_msg(self, msg);
                    bkerror!(r, tx, BKResponse::SendMsgRedactionError);
                }
                SetRoom(id) => {
                    let r = room::set_room(self, id);
                    bkerror!(r, tx, BKResponse::SetRoomError);
                }
                GetRoomAvatar(room) => {
                    let r = room::get_room_avatar(self, room);
                    bkerror!(r, tx, BKResponse::GetRoomAvatarError);
                }
                JoinRoom(room_id) => {
                    let r = room::join_room(self, room_id);
                    bkerror!(r, tx, BKResponse::JoinRoomError);
                }
                LeaveRoom(room_id) => {
                    let r = room::leave_room(self, room_id);
                    bkerror!(r, tx, BKResponse::LeaveRoomError);
                }
                MarkAsRead(room_id, evid) => {
                    let r = room::mark_as_read(self, room_id, evid);
                    bkerror!(r, tx, BKResponse::MarkAsReadError);
                }
                SetRoomName(room_id, name) => {
                    let r = room::set_room_name(self, room_id, name);
                    bkerror!(r, tx, BKResponse::SetRoomNameError);
                }
                SetRoomTopic(room_id, topic) => {
                    let r = room::set_room_topic(self, room_id, topic);
                    bkerror!(r, tx, BKResponse::SetRoomTopicError);
                }
                SetRoomAvatar(room_id, fname) => {
                    let r = room::set_room_avatar(self, room_id, fname);
                    bkerror!(r, tx, BKResponse::SetRoomAvatarError);
                }
                AttachFile(msg) => {
                    let r = room::attach_file(self, msg);
                    bkerror!(r, tx, BKResponse::AttachFileError);
                }
                NewRoom(name, privacy, internalid) => {
                    let r = room::new_room(self, name, privacy, internalid.clone());
                    r.or_else(|e| tx.send(BKResponse::NewRoomError(e, internalid)))
                        .unwrap();
                }
                DirectChat(user, internalid) => {
                    let r = room::direct_chat(self, user, internalid.clone());
                    r.or_else(|e| tx.send(BKResponse::NewRoomError(e, internalid)))
                        .unwrap();
                }
                AddToFav(room_id, tofav) => {
                    let r = room::add_to_fav(self, room_id, tofav);
                    bkerror!(r, tx, BKResponse::AddToFavError);
                }
                AcceptInv(room_id) => {
                    let r = room::join_room(self, room_id);
                    bkerror!(r, tx, BKResponse::AcceptInvError);
                }
                RejectInv(room_id) => {
                    let r = room::leave_room(self, room_id);
                    bkerror!(r, tx, BKResponse::RejectInvError);
                }
                Invite(room, userid) => {
                    let r = room::invite(self, room, userid);
                    bkerror!(r, tx, BKResponse::InviteError);
                }
                // Media module
                GetThumbAsync(media, ctx) => {
                    let r = media::get_thumb_async(self, media, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetMediaAsync(media, ctx) => {
                    let r = media::get_media_async(self, media, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetMediaListAsync(room_id, first_media_id, prev_batch, ctx) => {
                    let r =
                        media::get_media_list_async(self, room_id, first_media_id, prev_batch, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetMedia(media) => {
                    let r = media::get_media(self, media);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetMediaUrl(media, ctx) => {
                    let r = media::get_media_url(self, media.to_string(), ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                GetFileAsync(url, ctx) => {
                    let r = media::get_file_async(url, ctx);
                    bkerror!(r, tx, BKResponse::CommandError);
                }
                // Directory module
                DirectoryProtocols => {
                    let r = directory::protocols(self);
                    bkerror!(r, tx, BKResponse::DirectoryError);
                }
                DirectorySearch(dhs, dq, dtp, more) => {
                    let hs = Some(dhs).filter(|dhs| !dhs.is_empty());
                    let q = Some(dq).filter(|dq| !dq.is_empty());
                    let tp = Some(dtp).filter(|dtp| !dtp.is_empty());
                    let r = directory::room_search(self, hs, q, tp, more);
                    bkerror!(r, tx, BKResponse::DirectoryError);
                }
                // Stickers module
                ListStickers => {
                    let r = stickers::list(self);
                    bkerror!(r, tx, BKResponse::StickersError);
                }
                SendSticker(room, sticker) => {
                    let r = stickers::send(self, room, &sticker);
                    bkerror!(r, tx, BKResponse::StickersError);
                }
                PurchaseSticker(group) => {
                    let r = stickers::purchase(self, &group);
                    bkerror!(r, tx, BKResponse::StickersError);
                }
                // Internal commands
                ShutDown => {
                    tx.send(BKResponse::ShutDown).unwrap();
                    return false;
                }
            };
            true
        })
        .unwrap_or(false)
    }
}
