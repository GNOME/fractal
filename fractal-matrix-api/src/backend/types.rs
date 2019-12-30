use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use crate::error::Error;

use crate::r0::contact::get_identifiers::ThirdPartyIdentifier;
use crate::r0::thirdparty::get_supported_protocols::ProtocolInstance;
use crate::r0::AccessToken;
use crate::r0::Medium;
use crate::types::Event;
use crate::types::Member;
use crate::types::Message;
use crate::types::Room;

use crate::cache::CacheMap;
use url::Url;

#[derive(Debug)]
pub enum BKCommand {
    Login(String, String, Url, Url),
    Logout(Url, AccessToken),
    #[allow(dead_code)]
    Register(String, String, Url, Url),
    #[allow(dead_code)]
    Guest(Url, Url),
    GetUsername(Url, String),
    SetUserName(Url, AccessToken, String, String),
    GetThreePID(Url, AccessToken),
    GetTokenEmail(Url, AccessToken, Url, String, String),
    GetTokenPhone(Url, AccessToken, Url, String, String),
    SubmitPhoneToken(Url, String, String, String),
    AddThreePID(Url, AccessToken, Url, String, String),
    DeleteThreePID(Url, AccessToken, Medium, String),
    ChangePassword(Url, AccessToken, String, String, String),
    AccountDestruction(Url, AccessToken, String, String),
    GetAvatar(Url, String),
    SetUserAvatar(Url, AccessToken, String, String),
    Sync(Url, AccessToken, String, Option<String>, bool),
    GetRoomMembers(Url, AccessToken, String),
    GetRoomMessages(Url, AccessToken, String, String),
    GetRoomMessagesFromMsg(Url, AccessToken, String, Message),
    GetMessageContext(Url, AccessToken, Message),
    GetRoomAvatar(Url, AccessToken, String),
    GetThumbAsync(Url, String, Sender<String>),
    GetMediaAsync(Url, String, Sender<String>),
    GetMediaListAsync(
        Url,
        AccessToken,
        String,
        Option<String>,
        Option<String>,
        Sender<(Vec<Message>, String)>,
    ),
    GetFileAsync(Url, Sender<String>),
    GetAvatarAsync(Url, Option<Member>, Sender<String>),
    GetMedia(Url, String),
    GetMediaUrl(Url, String, Sender<String>),
    GetUserInfoAsync(Url, String, Option<Sender<(String, String)>>),
    GetUserNameAsync(Url, String, Sender<String>),
    SendMsg(Url, AccessToken, Message),
    SendMsgRedaction(Url, AccessToken, Message),
    SendTyping(Url, AccessToken, String, String),
    SetRoom(Url, AccessToken, String),
    ShutDown,
    DirectoryProtocols(Url, AccessToken),
    DirectorySearch(Url, AccessToken, String, String, String, bool),
    JoinRoom(Url, AccessToken, String),
    MarkAsRead(Url, AccessToken, String, String),
    LeaveRoom(Url, AccessToken, String),
    SetRoomName(Url, AccessToken, String, String),
    SetRoomTopic(Url, AccessToken, String, String),
    SetRoomAvatar(Url, AccessToken, String, String),
    AttachFile(Url, AccessToken, Message),
    NewRoom(Url, AccessToken, String, RoomType, String),
    DirectChat(Url, AccessToken, String, Member, String),
    AddToFav(Url, AccessToken, String, String, bool),
    AcceptInv(Url, AccessToken, String),
    RejectInv(Url, AccessToken, String),
    UserSearch(Url, AccessToken, String),
    Invite(Url, AccessToken, String, String),
    ChangeLanguage(AccessToken, Url, String, String, String),
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(String, AccessToken, Option<String>, Url, Url),
    Logout(Result<(), Error>),
    Name(Result<Option<String>, Error>),
    SetUserName(Result<String, Error>),
    GetThreePID(Result<Vec<ThirdPartyIdentifier>, Error>),
    GetTokenEmail(Result<(String, String), Error>),
    GetTokenPhone(Result<(String, String), Error>),
    SubmitPhoneToken(Result<(Option<String>, String), Error>),
    AddThreePID(Result<(), Error>),
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
    RemoveMessage(Result<(String, String), Error>),
    RoomName(String, String),
    RoomTopic(String, String),
    Media(Result<String, Error>),
    MediaUrl(Url),
    AttachedFile(Result<Message, Error>),
    NewRoom(Result<Room, Error>, String),
    AddedToFav(Result<(String, bool), Error>),
    RoomNotifications(String, i32, i32),
    UserSearch(Result<Vec<Member>, Error>),

    //errors
    LoginError(Error),
    GuestLoginError(Error),
    SendTypingError(Error),
    SetRoomError(Error),
    GetFileAsyncError(Error),
    InviteError(Error),
    ChangeLanguage(Result<(), Error>),
}

#[derive(Debug, Clone, Copy)]
pub enum RoomType {
    Public,
    Private,
}

pub struct BackendData {
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
