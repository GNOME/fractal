use std::sync::{Arc, Mutex, Condvar};
use std::sync::mpsc::Sender;

use error::Error;

use types::Message;
use types::Member;
use types::Protocol;
use types::Room;
use types::Event;
use types::StickerGroup;

use cache::CacheMap;


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
    GetAvatar,
    Sync,
    SyncForced,
    GetRoomMembers(String),
    GetRoomMessages(String),
    GetMessageContext(Message),
    GetRoomAvatar(String),
    GetThumbAsync(String, Sender<String>),
    GetAvatarAsync(Option<Member>, Sender<String>),
    GetMedia(String),
    GetUserInfoAsync(String, Sender<(String, String)>),
    SendMsg(Message),
    SetRoom(Room),
    ShutDown,
    DirectoryProtocols,
    DirectorySearch(String, String, bool),
    JoinRoom(String),
    MarkAsRead(String, String),
    LeaveRoom(String),
    SetRoomName(String, String),
    SetRoomTopic(String, String),
    SetRoomAvatar(String, String),
    AttachFile(String, String),
    AttachImage(String, Vec<u8>),
    Search(String, Option<String>),
    NewRoom(String, RoomType, String),
    DirectChat(Member, String),
    AddToFav(String, bool),
    AcceptInv(String),
    RejectInv(String),
    UserSearch(String),
    Invite(String, String),
    ListStickers,
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(String, String),
    Logout,
    Name(String),
    Avatar(String),
    Sync(String),
    Rooms(Vec<Room>, Option<Room>),
    NewRooms(Vec<Room>),
    RoomDetail(String, String, String),
    RoomAvatar(String, String),
    NewRoomAvatar(String),
    RoomMemberEvent(Event),
    RoomMessages(Vec<Message>),
    RoomMessagesInit(Vec<Message>),
    RoomMessagesTo(Vec<Message>),
    RoomMembers(String, Vec<Member>),
    SendMsg,
    DirectoryProtocols(Vec<Protocol>),
    DirectorySearch(Vec<Room>),
    JoinRoom,
    LeaveRoom,
    MarkedAsRead(String, String),
    SetRoomName,
    SetRoomTopic,
    SetRoomAvatar,
    RoomName(String, String),
    RoomTopic(String, String),
    Media(String),
    AttachedFile(Message),
    SearchEnd,
    NewRoom(Room, String),
    AddedToFav(String, bool),
    RoomNotifications(String, i32, i32),
    UserSearch(Vec<Member>),
    Stickers(Vec<StickerGroup>),

    //errors
    UserNameError(Error),
    AvatarError(Error),
    LoginError(Error),
    LogoutError(Error),
    GuestLoginError(Error),
    SyncError(Error),
    RoomDetailError(Error),
    RoomAvatarError(Error),
    RoomMessagesError(Error),
    RoomMembersError(Error),
    SendMsgError(Error),
    SetRoomError(Error),
    CommandError(Error),
    DirectoryError(Error),
    JoinRoomError(Error),
    MarkAsReadError(Error),
    LeaveRoomError(Error),
    SetRoomNameError(Error),
    SetRoomTopicError(Error),
    SetRoomAvatarError(Error),
    GetRoomAvatarError(Error),
    MediaError(Error),
    AttachFileError(Error),
    SearchError(Error),
    NewRoomError(Error, String),
    AddToFavError(Error),
    AcceptInvError(Error),
    RejectInvError(Error),
    InviteError(Error),
    StickersError(Error),
}

#[derive(Debug)]
pub enum RoomType {
    Public,
    Private,
}

pub struct BackendData {
    pub user_id: String,
    pub access_token: String,
    pub server_url: String,
    pub scalar_token: Option<String>,
    pub scalar_url: String,
    pub sticker_widget: Option<String>,
    pub since: String,
    pub rooms_since: String,
    pub join_to_room: String,
}

pub struct Backend {
    pub tx: Sender<BKResponse>,
    pub data: Arc<Mutex<BackendData>>,
    pub internal_tx: Option<Sender<BKCommand>>,

    // user info cache, uid -> (name, avatar)
    pub user_info_cache: CacheMap<Arc<Mutex<(String, String)>>>,
    // semaphore to limit the number of threads downloading images
    pub limit_threads: Arc<(Mutex<u8>, Condvar)>,
}

impl Clone for Backend {
    fn clone(&self) -> Backend {
        Backend {
            tx: self.tx.clone(),
            data: self.data.clone(),
            internal_tx: self.internal_tx.clone(),
            user_info_cache: self.user_info_cache.clone(),
            limit_threads: self.limit_threads.clone(),
        }
    }
}
