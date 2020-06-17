use ruma_identifiers::{DeviceId, EventId, RoomId, UserId};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::error::Error;

use crate::r0::AccessToken;
use crate::types::Event;
use crate::types::Message;
use crate::types::Room;

use url::Url;

#[derive(Debug)]
pub enum BKCommand {
    Sync(
        Url,
        AccessToken,
        UserId,
        Option<RoomId>,
        Option<String>,
        bool,
        u64,
    ),
    ShutDown,
    SendBKResponse(BKResponse),
}

#[derive(Debug)]
pub enum BKResponse {
    ShutDown,
    Token(UserId, AccessToken, Option<DeviceId>, Url, Url),
    Sync(Result<String, (Error, u64)>),
    Rooms(Result<(Vec<Room>, Option<Room>), Error>),
    UpdateRooms(Result<Vec<Room>, Error>),
    NewRoomAvatar(RoomId),
    RoomMemberEvent(Event),
    RoomMessages(Result<Vec<Message>, Error>),
    RoomMessagesInit(Vec<Message>),
    RemoveMessage(Result<(RoomId, EventId), Error>),
    RoomName(RoomId, String),
    RoomTopic(RoomId, String),
    MediaUrl(Url),
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
    JoinRoomError(Error),
    DirectorySearchError(Error),
    NewRoomError(Error, RoomId),
    RoomDetailError(Error),
    RoomAvatarError(Error),
    SentMsgError(Error),
    AttachedFileError(Error),
}

#[derive(Debug, Clone, Copy)]
pub enum RoomType {
    Public,
    Private,
}

#[derive(Clone, Debug)]
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

pub struct Backend {
    pub tx: Sender<BKResponse>,
}
