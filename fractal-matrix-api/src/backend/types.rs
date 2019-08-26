use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use crate::error::Error;

use crate::r0::contact::get_identifiers::ThirdPartyIdentifier;
use crate::r0::thirdparty::get_supported_protocols::ProtocolInstance;
use crate::r0::Medium;
use crate::types::Event;
use crate::types::Member;
use crate::types::Message;
use crate::types::Room;
use crate::types::Sticker;
use crate::types::StickerGroup;

use crate::cache::CacheMap;
use url::Url;

#[derive(Debug)]
pub enum BKCommand {
    Login(String, String, String),
    SetToken(String, String, String),
    Logout,
    #[allow(dead_code)]
    Register(String, String, String),
    #[allow(dead_code)]
    Guest(String),
    GetUsername,
    SetUserName(String),
    GetThreePID,
    GetTokenEmail(String, String, String),
    GetTokenPhone(String, String, String),
    SubmitPhoneToken(String, String, String, String),
    AddThreePID(String, String, String),
    DeleteThreePID(Medium, String),
    ChangePassword(String, String, String),
    AccountDestruction(String, String, bool),
    GetAvatar,
    SetUserAvatar(String),
    Sync(Option<String>, bool),
    SyncForced,
    GetRoomMembers(String),
    GetRoomMessages(String, String),
    GetRoomMessagesFromMsg(String, Message),
    GetMessageContext(Message),
    GetRoomAvatar(String),
    GetThumbAsync(String, Sender<String>),
    GetMediaAsync(String, Sender<String>),
    GetMediaListAsync(
        String,
        Option<String>,
        Option<String>,
        Sender<(Vec<Message>, String)>,
    ),
    GetFileAsync(String, Sender<String>),
    GetAvatarAsync(Option<Member>, Sender<String>),
    GetMedia(String),
    GetMediaUrl(String, Sender<String>),
    GetUserInfoAsync(String, Option<Sender<(String, String)>>),
    GetUserNameAsync(String, Sender<String>),
    SendMsg(Message),
    SendMsgRedaction(Message),
    SendTyping(String),
    SetRoom(String),
    ShutDown,
    DirectoryProtocols,
    DirectorySearch(String, String, String, bool),
    JoinRoom(String),
    MarkAsRead(String, String),
    LeaveRoom(String),
    SetRoomName(String, String),
    SetRoomTopic(String, String),
    SetRoomAvatar(String, String),
    AttachFile(Message),
    NewRoom(String, RoomType, String),
    DirectChat(Member, String),
    AddToFav(String, bool),
    AcceptInv(String),
    RejectInv(String),
    UserSearch(String),
    Invite(String, String),
    ListStickers,
    SendSticker(String, Sticker),
    PurchaseSticker(StickerGroup),
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(String, String, Option<String>),
    Logout(Result<(), Error>),
    Name(Result<String, Error>),
    SetUserName(Result<String, Error>),
    GetThreePID(Result<Vec<ThirdPartyIdentifier>, Error>),
    GetTokenEmail(Result<(String, String), Error>),
    GetTokenPhone(Result<(String, String), Error>),
    SubmitPhoneToken(Result<(Option<String>, String), Error>),
    AddThreePID(Result<String, Error>),
    DeleteThreePID(Result<(), Error>),
    ChangePassword(Result<(), Error>),
    AccountDestruction(Result<(), Error>),
    Avatar(Result<String, Error>),
    SetUserAvatar(Result<String, Error>),
    Sync(Result<String, Error>),
    Rooms(Vec<Room>, Option<Room>),
    UpdateRooms(Vec<Room>),
    RoomDetail(Result<(String, String, String), Error>),
    RoomAvatar(Result<(String, Option<Url>), Error>),
    NewRoomAvatar(String),
    RoomMemberEvent(Event),
    RoomMessages(Vec<Message>),
    RoomMessagesInit(Vec<Message>),
    RoomMessagesTo(Result<(Vec<Message>, String, Option<String>), Error>),
    RoomMembers(Result<(String, Vec<Member>), Error>),
    SentMsg(Result<(String, String), Error>),
    SentMsgRedaction(Result<(String, String), Error>),
    DirectoryProtocols(Result<Vec<ProtocolInstance>, Error>),
    DirectorySearch(Result<Vec<Room>, Error>),
    JoinRoom(Result<(), Error>),
    LeaveRoom(Result<(), Error>),
    MarkedAsRead(Result<(String, String), Error>),
    SetRoomName(Result<(), Error>),
    SetRoomTopic(Result<(), Error>),
    SetRoomAvatar(Result<(), Error>),
    RoomName(String, String),
    RoomTopic(String, String),
    Media(Result<String, Error>),
    MediaUrl(Url),
    AttachedFile(Result<Message, Error>),
    NewRoom(Result<Room, Error>, String),
    AddedToFav(Result<(String, bool), Error>),
    RoomNotifications(String, i32, i32),
    UserSearch(Result<Vec<Member>, Error>),
    Stickers(Result<Vec<StickerGroup>, Error>),

    //errors
    LoginError(Error),
    GuestLoginError(Error),
    SendTypingError(Error),
    SetRoomError(Error),
    GetFileAsyncError(Error),
    InviteError(Error),
}

#[derive(Debug, Clone, Copy)]
pub enum RoomType {
    Public,
    Private,
}

pub struct BackendData {
    pub user_id: String,
    pub access_token: String,
    pub server_url: Url,
    pub scalar_token: Option<String>,
    pub scalar_url: Url,
    pub sticker_widget: Option<String>,
    pub since: Option<String>,
    pub rooms_since: String,
    pub join_to_room: String,
    pub m_direct: HashMap<String, Vec<String>>,
}

#[derive(Clone)]
pub struct Backend {
    pub tx: Sender<BKResponse>,
    pub data: Arc<Mutex<BackendData>>,
    pub internal_tx: Option<Sender<BKCommand>>,

    // user info cache, uid -> (name, avatar)
    pub user_info_cache: CacheMap<Arc<Mutex<(String, String)>>>,
    // semaphore to limit the number of threads downloading images
    pub limit_threads: Arc<(Mutex<u8>, Condvar)>,
}
