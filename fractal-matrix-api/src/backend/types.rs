use ruma_identifiers::{RoomId, UserId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::error::Error;

use crate::r0::contact::get_identifiers::ThirdPartyIdentifier;
use crate::r0::thirdparty::get_supported_protocols::ProtocolInstance;
use crate::r0::AccessToken;
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
    Register(String, String, Url, Url),
    Guest(Url, Url),
    Sync(Url, AccessToken, UserId, Option<String>, bool),
    GetRoomMembers(Url, AccessToken, RoomId),
    GetRoomMessages(Url, AccessToken, RoomId, String),
    GetRoomMessagesFromMsg(Url, AccessToken, RoomId, Message),
    GetMessageContext(Url, AccessToken, Message),
    GetRoomAvatar(Url, AccessToken, RoomId),
    GetThumbAsync(Url, String, Sender<Result<String, Error>>),
    GetMediaAsync(Url, String, Sender<Result<String, Error>>),
    GetMediaListAsync(
        Url,
        AccessToken,
        RoomId,
        Option<String>,
        Option<String>,
        Sender<(Vec<Message>, String)>,
    ),
    GetAvatarAsync(Url, Option<Member>, Sender<String>),
    GetMedia(Url, String),
    GetUserInfoAsync(Url, UserId, Option<Sender<(String, String)>>),
    SendMsg(Url, AccessToken, Message),
    SendMsgRedaction(Url, AccessToken, Message),
    SendTyping(Url, AccessToken, UserId, RoomId),
    SetRoom(Url, AccessToken, RoomId),
    ShutDown,
    DirectoryProtocols(Url, AccessToken),
    DirectorySearch(Url, AccessToken, String, String, String, bool),
    JoinRoom(Url, AccessToken, RoomId),
    MarkAsRead(Url, AccessToken, RoomId, String),
    LeaveRoom(Url, AccessToken, RoomId),
    SetRoomName(Url, AccessToken, RoomId, String),
    SetRoomTopic(Url, AccessToken, RoomId, String),
    SetRoomAvatar(Url, AccessToken, RoomId, String),
    AttachFile(Url, AccessToken, Message),
    NewRoom(Url, AccessToken, String, RoomType, RoomId),
    DirectChat(Url, AccessToken, UserId, Member, RoomId),
    AddToFav(Url, AccessToken, UserId, RoomId, bool),
    AcceptInv(Url, AccessToken, RoomId),
    RejectInv(Url, AccessToken, RoomId),
    Invite(Url, AccessToken, RoomId, UserId),
    ChangeLanguage(AccessToken, Url, UserId, RoomId, String),
    SendBKResponse(BKResponse),
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(UserId, AccessToken, Option<String>, Url, Url),
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
    Avatar(Result<PathBuf, Error>),
    SetUserAvatar(Result<PathBuf, Error>),
    Sync(Result<String, Error>),
    Rooms(Result<(Vec<Room>, Option<Room>), Error>),
    UpdateRooms(Result<Vec<Room>, Error>),
    RoomDetail(Result<(RoomId, String, String), Error>),
    RoomAvatar(Result<(RoomId, Option<Url>), Error>),
    NewRoomAvatar(RoomId),
    RoomMemberEvent(Event),
    RoomMessages(Result<Vec<Message>, Error>),
    RoomMessagesInit(Vec<Message>),
    RoomMessagesTo(Result<(Vec<Message>, RoomId, Option<String>), Error>),
    RoomMembers(Result<(RoomId, Vec<Member>), Error>),
    SentMsg(Result<(String, String), Error>),
    SentMsgRedaction(Result<(String, String), Error>),
    DirectoryProtocols(Result<Vec<ProtocolInstance>, Error>),
    DirectorySearch(Result<Vec<Room>, Error>),
    JoinRoom(Result<(), Error>),
    LeaveRoom(Result<(), Error>),
    MarkedAsRead(Result<(RoomId, String), Error>),
    SetRoomName(Result<(), Error>),
    SetRoomTopic(Result<(), Error>),
    SetRoomAvatar(Result<(), Error>),
    RemoveMessage(Result<(RoomId, String), Error>),
    RoomName(RoomId, String),
    RoomTopic(RoomId, String),
    Media(Result<String, Error>),
    MediaUrl(Url),
    AttachedFile(Result<Message, Error>),
    NewRoom(Result<Room, Error>, RoomId),
    AddedToFav(Result<(RoomId, bool), Error>),
    RoomNotifications(RoomId, i32, i32),
    UserSearch(Result<Vec<Member>, Error>),

    //errors
    LoginError(Error),
    GuestLoginError(Error),
    SendTypingError(Error),
    SetRoomError(Error),
    InviteError(Error),
    ChangeLanguage(Result<(), Error>),
}

#[derive(Debug, Clone, Copy)]
pub enum RoomType {
    Public,
    Private,
}

pub struct ThreadPool {
    thread_count: Arc<(Mutex<u8>, Condvar)>,
    limit: u8,
}

impl ThreadPool {
    pub fn new(limit: u8) -> Self {
        ThreadPool {
            thread_count: Arc::new((Mutex::new(0), Condvar::new())),
            limit,
        }
    }

    pub fn run<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let thread_count = self.thread_count.clone();
        let limit = self.limit;
        thread::spawn(move || {
            // waiting, less than {limit} threads at the same time
            let &(ref num, ref cvar) = &*thread_count;
            {
                let mut start = num.lock().unwrap();
                while *start >= limit {
                    start = cvar.wait(start).unwrap()
                }
                *start += 1;
            }

            func();

            // freeing the cvar for new threads
            {
                let mut counter = num.lock().unwrap();
                *counter -= 1;
            }
            cvar.notify_one();
        });
    }
}

pub struct BackendData {
    pub rooms_since: String,
    pub join_to_room: Option<RoomId>,
    pub m_direct: HashMap<UserId, Vec<RoomId>>,
}

pub struct Backend {
    pub tx: Sender<BKResponse>,
    pub data: Arc<Mutex<BackendData>>,

    // user info cache, uid -> (name, avatar)
    pub user_info_cache: CacheMap<UserId, Arc<Mutex<(String, String)>>>,
    pub thread_pool: ThreadPool,
}
