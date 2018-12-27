mod directory;
mod media;
mod register;
mod room;
mod stickers;
mod sync;
mod types;
mod user;

pub use self::types::{BKCommand, BKResponse, Backend, BackendData, RoomType};

use self::types::BKCommand::*;
use crate::{cache::CacheMap, util::client_url};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, RecvError, Sender},
        Arc, Condvar, Mutex,
    },
    thread,
};
use url::Url;

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
            tx,
            internal_tx: None,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(60 * 60),
            limit_threads: Arc::new((Mutex::new(0u8), Condvar::new())),
        }
    }

    fn get_base_url(&self) -> Url {
        let s = self.data.lock().unwrap().server_url.clone();
        Url::parse(&s).unwrap()
    }

    fn url(&self, path: &str, mut params: Vec<(&str, String)>) -> Url {
        let base = self.get_base_url();
        let tk = self.data.lock().unwrap().access_token.clone();

        params.push(("access_token", tk));

        client_url(&base, path, &params).unwrap()
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

    fn command_recv(&mut self, cmd: Result<BKCommand, RecvError>) -> bool {
        cmd.map(|cmd| {
            match cmd {
                // Register module
                Login(user, passwd, server) => self.login(user, passwd, server),
                Logout => self.logout(),
                Register(user, passwd, server) => self.register(user, passwd, server),
                Guest(server) => self.guest(server),
                SetToken(token, uid, server) => self.set_token(token, uid, server),
                // User module
                GetUsername => self.get_username(),
                SetUserName(name) => self.set_username(name),
                GetThreePID => self.get_threepid(),
                GetTokenEmail(identity, email, client_secret) => {
                    self.get_email_token(identity, email, client_secret)
                }
                GetTokenPhone(identity, phone, client_secret) => {
                    self.get_phone_token(identity, phone, client_secret)
                }
                SubmitPhoneToken(identity, client_secret, sid, token) => {
                    self.submit_phone_token(identity, client_secret, sid, token)
                }
                AddThreePID(identity, client_secret, sid) => {
                    self.add_threepid(identity, client_secret, sid)
                }
                DeleteThreePID(medium, address) => self.delete_threepid(medium, address),
                ChangePassword(username, old_password, new_password) => {
                    self.change_password(username, old_password, new_password)
                }
                AccountDestruction(username, password, flag) => {
                    self.account_destruction(username, password, flag)
                }
                GetAvatar => self.get_avatar(),
                SetUserAvatar(file) => self.set_user_avatar(file),
                GetAvatarAsync(member, ctx) => self.get_avatar_async(member, ctx),
                GetUserInfoAsync(sender, ctx) => self.get_user_info_async(sender, ctx),
                GetUserNameAsync(sender, ctx) => self.get_username_async(sender, ctx),
                UserSearch(term) => self.user_search(term),
                // Sync module
                Sync(since, initial) => self.sync(since, initial),
                SyncForced => self.sync_forced(),
                // Room module
                GetRoomMembers(room) => self.get_room_members(room),
                GetRoomMessages(room, from) => self.get_room_messages(room, from),
                GetRoomMessagesFromMsg(room, from) => self.get_room_messages_from_msg(room, from),
                GetMessageContext(message) => self.get_message_context(message),
                SendMsg(msg) => self.send_msg(msg),
                SendMsgRedaction(msg) => self.send_msg_redaction(msg),
                SetRoom(id) => self.set_room(id),
                GetRoomAvatar(room) => self.get_room_avatar(room),
                JoinRoom(room_id) => self.join_room(room_id),
                LeaveRoom(room_id) => self.leave_room(room_id),
                MarkAsRead(room_id, evid) => self.mark_as_read(room_id, evid),
                SetRoomName(room_id, name) => self.set_room_name(room_id, name),
                SetRoomTopic(room_id, topic) => self.set_room_topic(room_id, topic),
                SetRoomAvatar(room_id, fname) => self.set_room_avatar(room_id, fname),
                AttachFile(msg) => self.attach_file(msg),
                NewRoom(name, privacy, internalid) => self.new_room(name, privacy, internalid),
                DirectChat(user, internalid) => self.direct_chat(user, internalid),
                AddToFav(room_id, tofav) => self.add_to_fav(room_id, tofav),
                AcceptInv(room_id) => self.accept_inv(room_id),
                RejectInv(room_id) => self.reject_inv(room_id),
                Invite(room, userid) => self.invite(room, userid),
                // Media module
                GetThumbAsync(media, ctx) => self.get_thumb_async(media, ctx),
                GetMediaAsync(media, ctx) => self.get_media_async(media, ctx),
                GetMediaListAsync(room_id, first_media_id, prev_batch, ctx) => {
                    self.get_media_list_async(room_id, first_media_id, prev_batch, ctx)
                }
                GetMedia(media) => self.get_media(media),
                GetMediaUrl(media, ctx) => self.get_media_url(media, ctx),
                GetFileAsync(url, ctx) => self.get_file_async(url, ctx),
                // Directory module
                DirectoryProtocols => self.directory_protocols(),
                DirectorySearch(hs, q, tp, more) => self.directory_search(hs, q, tp, more),
                // Stickers module
                ListStickers => self.list_stickers(),
                SendSticker(room_id, sticker) => self.send_sticker(room_id, sticker),
                PurchaseSticker(group) => self.purchase_sticker(group),
                // Internal commands
                ShutDown => {
                    self.tx.clone().send(BKResponse::ShutDown).unwrap();
                    return false;
                }
            };
            true
        })
        .unwrap_or(false)
    }
}
