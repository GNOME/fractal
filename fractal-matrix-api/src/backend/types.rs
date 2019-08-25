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
    Logout,
    Name(String),
    SetUserName(String),
    GetThreePID(Vec<ThirdPartyIdentifier>),
    GetTokenEmail(String, String),
    GetTokenPhone(String, String),
    SubmitPhoneToken(Option<String>, String),
    AddThreePID(String),
    GetTokenPhoneUsed,
    GetTokenEmailUsed,
    DeleteThreePID,
    ChangePassword,
    AccountDestruction,
    Avatar(String),
    SetUserAvatar(String),
    Sync(String),
    Rooms(Vec<Room>, Option<Room>),
    UpdateRooms(Vec<Room>),
    RoomDetail(String, String, String),
    RoomAvatar(String, Option<Url>),
    NewRoomAvatar(String),
    RoomMemberEvent(Event),
    RoomMessages(Vec<Message>),
    RoomMessagesInit(Vec<Message>),
    RoomMessagesTo(Vec<Message>, String, Option<String>),
    RoomMembers(String, Vec<Member>),
    SentMsg(String, String),
    SentMsgRedaction(String, String),
    DirectoryProtocols(Result<Vec<ProtocolInstance>, Error>),
    DirectorySearch(Result<Vec<Room>, Error>),
    JoinRoom,
    LeaveRoom,
    MarkedAsRead(String, String),
    SetRoomName,
    SetRoomTopic,
    SetRoomAvatar,
    RoomName(String, String),
    RoomTopic(String, String),
    Media(String),
    MediaUrl(Url),
    AttachedFile(Message),
    NewRoom(Room, String),
    AddedToFav(String, bool),
    RoomNotifications(String, i32, i32),
    UserSearch(Vec<Member>),
    Stickers(Vec<StickerGroup>),

    //errors
    UserNameError(Error),
    SetUserNameError(Error),
    GetThreePIDError(Error),
    GetTokenEmailError(Error),
    GetTokenPhoneError(Error),
    SubmitPhoneTokenError(Error),
    AddThreePIDError(Error),
    DeleteThreePIDError(Error),
    ChangePasswordError(Error),
    AccountDestructionError(Error),
    AvatarError(Error),
    SetUserAvatarError(Error),
    LoginError(Error),
    LogoutError(Error),
    GuestLoginError(Error),
    SyncError(Error),
    RoomDetailError(Error),
    RoomAvatarError(Error),
    RoomMessagesError(Error),
    RoomMembersError(Error),
    SendMsgError(Error),
    SendMsgRedactionError(Error),
    SendTypingError(Error),
    SetRoomError(Error),
    CommandError(Error),
    JoinRoomError(Error),
    MarkAsReadError(Error),
    LeaveRoomError(Error),
    SetRoomNameError(Error),
    SetRoomTopicError(Error),
    SetRoomAvatarError(Error),
    GetRoomAvatarError(Error),
    MediaError(Error),
    AttachFileError(Error),
    NewRoomError(Error, String),
    AddToFavError(Error),
    AcceptInvError(Error),
    RejectInvError(Error),
    InviteError(Error),
    StickersError(Error),
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
