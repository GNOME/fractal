use ruma_identifiers::{RoomId, UserId};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::error::Error;

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
    Register(String, String, Url, Url),
    Guest(Url, Url),
    Sync(Url, AccessToken, UserId, Option<String>, bool),
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
    GetUserInfoAsync(Url, UserId, Option<Sender<(String, String)>>),
    SetRoom(Url, AccessToken, RoomId),
    ShutDown,
    DirectorySearch(Url, AccessToken, String, String, String, bool),
    JoinRoom(Url, AccessToken, RoomId),
    AttachFile(Url, AccessToken, Message),
    DirectChat(Url, AccessToken, UserId, Member, RoomId),
    AcceptInv(Url, AccessToken, RoomId),
    SendBKResponse(BKResponse),
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(UserId, AccessToken, Option<String>, Url, Url),
    Sync(Result<String, Error>),
    Rooms(Result<(Vec<Room>, Option<Room>), Error>),
    UpdateRooms(Result<Vec<Room>, Error>),
    RoomDetail(Result<(RoomId, String, String), Error>),
    RoomAvatar(Result<(RoomId, Option<Url>), Error>),
    NewRoomAvatar(RoomId),
    RoomMemberEvent(Event),
    RoomMessages(Result<Vec<Message>, Error>),
    RoomMessagesInit(Vec<Message>),
    SentMsg(Result<(String, String), Error>),
    DirectorySearch(Result<Vec<Room>, Error>),
    JoinRoom(Result<(), Error>),
    RemoveMessage(Result<(RoomId, String), Error>),
    RoomName(RoomId, String),
    RoomTopic(RoomId, String),
    MediaUrl(Url),
    AttachedFile(Result<Message, Error>),
    NewRoom(Result<Room, Error>, RoomId),
    RoomNotifications(RoomId, i32, i32),

    //errors
    LoginError(Error),
    GuestLoginError(Error),
    SendTypingError(Error),
    SetRoomError(Error),
    InviteError(Error),
    ChangeLanguageError(Error),
    NameError(Error),
    AvatarError(Error),
    MarkedAsReadError(Error),
    UserSearchError(Error),
    LogoutError(Error),
    LeaveRoomError(Error),
    DirectoryProtocolsError(Error),
    RoomMembersError(Error),
    AddedToFavError(Error),
    GetThreePIDError(Error),
    AddThreePIDError(Error),
    SubmitPhoneTokenError(Error),
    SetUserNameError(Error),
    ChangePasswordError(Error),
    AccountDestructionError(Error),
    DeleteThreePIDError(Error),
    GetTokenPhoneError(Error),
    GetTokenEmailError(Error),
    SetRoomNameError(Error),
    SetRoomTopicError(Error),
    SetUserAvatarError(Error),
    SetRoomAvatarError(Error),
    RoomMessagesToError(Error),
    MediaError(Error),
    SentMsgRedactionError(Error),
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
