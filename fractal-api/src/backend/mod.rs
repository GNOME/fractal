extern crate url;

use globals;
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use self::url::Url;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;

use error::Error;

use util::build_url;
use util::json_q;
use cache::CacheMap;

mod types;
mod register;
mod user;
mod room;
mod sync;
mod media;
mod directory;
mod stickers;

pub use self::types::BKResponse;
pub use self::types::BKCommand;

pub use self::types::Backend;
pub use self::types::BackendData;

pub use self::types::RoomType;


impl Backend {
    pub fn new(tx: Sender<BKResponse>) -> Backend {
        let data = BackendData {
            user_id: String::from("Guest"),
            access_token: String::from(""),
            server_url: String::from("https://matrix.org"),
            scalar_token: None,
            scalar_url: String::from("https://scalar.vector.im"),
            sticker_widget: None,
            since: String::from(""),
            rooms_since: String::from(""),
            join_to_room: String::from(""),
        };
        Backend {
            tx: tx,
            internal_tx: None,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(60*60),
            limit_threads: Arc::new((Mutex::new(0u8), Condvar::new())),
        }
    }

    fn get_base_url(&self) -> Result<Url, Error> {
        let s = self.data.lock().unwrap().server_url.clone();
        let url = Url::parse(&s)?;
        Ok(url)
    }

    fn url(&self, path: &str, params: Vec<(&str, String)>) -> Result<Url, Error> {
        let base = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();

        let mut params2 = params.to_vec();
        params2.push(("access_token", tk.clone()));

        client_url!(&base, path, params2)
    }

    fn get_scalar_token(&self) -> Result<String, Error> {
        let s = self.data.lock().unwrap().scalar_url.clone();
        let uid = self.data.lock().unwrap().user_id.clone();

        let url = self.url(&format!("user/{}/openid/request_token", uid), vec![])?;
        let js = json_q("post", &url, &json!({}), globals::TIMEOUT)?;

        let vurl = Url::parse(&format!("{}/api/register", s))?;
        let js = json_q("post", &vurl, &js, globals::TIMEOUT)?;

        match js["scalar_token"].as_str() {
            Some(st) => {
                self.data.lock().unwrap().scalar_token = Some(st.to_string());
                Ok(st.to_string())
            }
            None => Err(Error::BackendError),
        }
    }

    fn vurl(&self, path: &str, params: Vec<(&str, String)>) -> Result<Url, Error> {
        let s = self.data.lock().unwrap().scalar_url.clone();
        let base = Url::parse(&s)?;
        let token = self.data.lock().unwrap().scalar_token.clone();
        let tk = match token {
            None => self.get_scalar_token()?,
            Some(t) => t.clone(),
        };

        let mut params2 = params.to_vec();
        params2.push(("scalar_token", tk));

        scalar_url!(&base, path, params2)
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
                let r = register::login(self, user, passwd, server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Logout) => {
                let r = register::logout(self);
                bkerror!(r, tx, BKResponse::LogoutError);
            }
            Ok(BKCommand::Register(user, passwd, server)) => {
                let r = register::register(self, user, passwd, server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Guest(server)) => {
                let r = register::guest(self, server);
                bkerror!(r, tx, BKResponse::GuestLoginError);
            }
            Ok(BKCommand::SetToken(token, uid, server)) => {
                let r = register::set_token(self, token, uid, server);
                bkerror!(r, tx, BKResponse::LoginError);
            }

            // User module

            Ok(BKCommand::GetUsername) => {
                let r = user::get_username(self);
                bkerror!(r, tx, BKResponse::UserNameError);
            }
            Ok(BKCommand::GetAvatar) => {
                let r = user::get_avatar(self);
                bkerror!(r, tx, BKResponse::AvatarError);
            }
            Ok(BKCommand::GetAvatarAsync(member, ctx)) => {
                let r = user::get_avatar_async(self, member, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetUserInfoAsync(sender, ctx)) => {
                let r = user::get_user_info_async(self, &sender, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::UserSearch(term)) => {
                let r = user::search(self, term);
                bkerror!(r, tx, BKResponse::CommandError);
            }

            // Sync module

            Ok(BKCommand::Sync) => {
                let r = sync::sync(self);
                bkerror!(r, tx, BKResponse::SyncError);
            }
            Ok(BKCommand::SyncForced) => {
                let r = sync::force_sync(self);
                bkerror!(r, tx, BKResponse::SyncError);
            }

            // Room module

            Ok(BKCommand::GetRoomMembers(room)) => {
                let r = room::get_room_members(self, room);
                bkerror!(r, tx, BKResponse::RoomMembersError);
            }
            Ok(BKCommand::GetRoomMessages(room)) => {
                let r = room::get_room_messages(self, room);
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
            Ok(BKCommand::SetRoom(room)) => {
                let r = room::set_room(self, room);
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
                let r = room::leave_room(self, roomid);
                bkerror!(r, tx, BKResponse::LeaveRoomError);
            }
            Ok(BKCommand::MarkAsRead(roomid, evid)) => {
                let r = room::mark_as_read(self, roomid, evid);
                bkerror!(r, tx, BKResponse::MarkAsReadError);
            }
            Ok(BKCommand::SetRoomName(roomid, name)) => {
                let r = room::set_room_name(self, roomid, name);
                bkerror!(r, tx, BKResponse::SetRoomNameError);
            }
            Ok(BKCommand::SetRoomTopic(roomid, topic)) => {
                let r = room::set_room_topic(self, roomid, topic);
                bkerror!(r, tx, BKResponse::SetRoomTopicError);
            }
            Ok(BKCommand::SetRoomAvatar(roomid, fname)) => {
                let r = room::set_room_avatar(self, roomid, fname);
                bkerror!(r, tx, BKResponse::SetRoomAvatarError);
            }
            Ok(BKCommand::AttachFile(roomid, fname)) => {
                let r = room::attach_file(self, roomid, fname);
                bkerror!(r, tx, BKResponse::AttachFileError);
            }
            Ok(BKCommand::AttachImage(roomid, image)) => {
                let r = room::attach_image(self, roomid, image);
                bkerror!(r, tx, BKResponse::AttachFileError);
            }
            Ok(BKCommand::NewRoom(name, privacy, internalid)) => {
                let r = room::new_room(self, name, privacy, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoomError(e, internalid)).unwrap();
                }
            }
            Ok(BKCommand::DirectChat(user, internalid)) => {
                let r = room::direct_chat(self, user, internalid.clone());
                if let Err(e) = r {
                    tx.send(BKResponse::NewRoomError(e, internalid)).unwrap();
                }
            }
            Ok(BKCommand::AddToFav(roomid, tofav)) => {
                let r = room::add_to_fav(self, roomid, tofav);
                bkerror!(r, tx, BKResponse::AddToFavError);
            }
            Ok(BKCommand::Search(roomid, term)) => {
                let r = room::search(self, roomid, term);
                bkerror!(r, tx, BKResponse::SearchError);
            }
            Ok(BKCommand::AcceptInv(roomid)) => {
                let r = room::join_room(self, roomid);
                bkerror!(r, tx, BKResponse::AcceptInvError);
            }
            Ok(BKCommand::RejectInv(roomid)) => {
                let r = room::leave_room(self, roomid);
                bkerror!(r, tx, BKResponse::RejectInvError);
            }
            Ok(BKCommand::Invite(room, userid)) => {
                let r = room::invite(self, room, userid);
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
            Ok(BKCommand::GetMedia(media)) => {
                let r = media::get_media(self, media);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetFileAsync(url, ctx)) => {
                let r = media::get_file_async(url, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }

            // Directory module

            Ok(BKCommand::DirectoryProtocols) => {
                let r = directory::protocols(self);
                bkerror!(r, tx, BKResponse::DirectoryError);
            }
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
                let r = stickers::send(self, room, &sticker);
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
