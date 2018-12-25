use crate::app::App;
use crate::i18n::i18n;
use log::{error, info};

use crate::actions::AppState;
use crate::appop::RoomPanel;

use glib;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::thread;

use crate::backend::BKResponse;
use fractal_api::error::Error;

use std::sync::mpsc::RecvError;

pub fn backend_loop(rx: Receiver<BKResponse>) {
    thread::spawn(move || {
        let mut shutting_down = false;
        loop {
            let recv = rx.recv();

            if let Err(RecvError) = recv {
                // stopping this backend loop thread
                break;
            }

            if shutting_down {
                // ignore this event, we're shutting down this thread
                continue;
            }

            match recv {
                Err(RecvError) => {
                    break;
                }
                Ok(BKResponse::ShutDown) => {
                    shutting_down = true;
                }
                Ok(BKResponse::Token(uid, tk, dev)) => {
                    APPOP!(bk_login, (uid, tk, dev));
                }
                Ok(BKResponse::Logout) => {
                    APPOP!(bk_logout);
                }
                Ok(BKResponse::Name(username)) => {
                    let u = Some(username);
                    APPOP!(set_username, (u));
                }
                Ok(BKResponse::GetThreePID(list)) => {
                    let l = Some(list);
                    APPOP!(set_three_pid, (l));
                }
                Ok(BKResponse::GetTokenEmail(sid, secret)) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_email, (sid, secret));
                }
                Ok(BKResponse::GetTokenPhone(sid, secret)) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_phone, (sid, secret));
                }
                Ok(BKResponse::GetTokenEmailUsed) => {
                    let error = i18n("Email is already in use");
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse::GetTokenPhoneUsed) => {
                    let error = i18n("Phone number is already in use");
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse::SubmitPhoneToken(sid, secret)) => {
                    let secret = Some(secret);
                    APPOP!(valid_phone_token, (sid, secret));
                }
                Ok(BKResponse::AddThreePID(list)) => {
                    let l = Some(list);
                    APPOP!(added_three_pid, (l));
                }
                Ok(BKResponse::DeleteThreePID) => {
                    APPOP!(get_three_pid);
                }
                Ok(BKResponse::ChangePassword) => {
                    APPOP!(password_changed);
                }
                Ok(BKResponse::SetUserName(username)) => {
                    let u = Some(username);
                    APPOP!(show_new_username, (u));
                }
                Ok(BKResponse::AccountDestruction) => {
                    APPOP!(account_destruction_logoff);
                }
                Ok(BKResponse::Avatar(path)) => {
                    let av = Some(path);
                    APPOP!(set_avatar, (av));
                }
                Ok(BKResponse::SetUserAvatar(path)) => {
                    let av = Some(path);
                    APPOP!(show_new_avatar, (av));
                }
                Ok(BKResponse::Sync(since)) => {
                    info!("SYNC");
                    let s = Some(since);
                    APPOP!(synced, (s));
                }
                Ok(BKResponse::Rooms(rooms, default)) => {
                    let clear_room_list = true;
                    APPOP!(set_rooms, (rooms, clear_room_list));
                    // Open the newly joined room
                    if let Some(room) = default {
                        let room_id = room.id;
                        APPOP!(set_active_room_by_id, (room_id));
                    }
                }
                Ok(BKResponse::NewRooms(rooms)) => {
                    let clear_room_list = false;
                    APPOP!(set_rooms, (rooms, clear_room_list));
                }
                Ok(BKResponse::RoomDetail(room, key, value)) => {
                    let v = Some(value);
                    APPOP!(set_room_detail, (room, key, v));
                }
                Ok(BKResponse::RoomAvatar(room, avatar)) => {
                    APPOP!(set_room_avatar, (room, avatar));
                }
                Ok(BKResponse::RoomMembers(room, members)) => {
                    APPOP!(set_room_members, (room, members));
                }
                Ok(BKResponse::RoomMessages(msgs)) => {
                    APPOP!(show_room_messages, (msgs));
                }
                Ok(BKResponse::RoomMessagesTo(msgs, room, prev_batch)) => {
                    APPOP!(show_room_messages_top, (msgs, room, prev_batch));
                }
                Ok(BKResponse::SentMsg(txid, evid)) => {
                    APPOP!(msg_sent, (txid, evid));
                    let initial = false;
                    APPOP!(sync, (initial));
                }
                Ok(BKResponse::DirectoryProtocols(protocols)) => {
                    APPOP!(set_protocols, (protocols));
                }
                Ok(BKResponse::DirectorySearch(rooms)) => {
                    APPOP!(set_directory_rooms, (rooms));
                }

                Ok(BKResponse::JoinRoom) => {
                    APPOP!(reload_rooms);
                }
                Ok(BKResponse::LeaveRoom) => {}
                Ok(BKResponse::SetRoomName) => {
                    APPOP!(show_new_room_name);
                }
                Ok(BKResponse::SetRoomTopic) => {
                    APPOP!(show_new_room_topic);
                }
                Ok(BKResponse::SetRoomAvatar) => {
                    APPOP!(show_new_room_avatar);
                }
                Ok(BKResponse::MarkedAsRead(r, _)) => {
                    APPOP!(clear_room_notifications, (r));
                }
                Ok(BKResponse::RoomNotifications(r, n, h)) => {
                    APPOP!(set_room_notifications, (r, n, h));
                }

                Ok(BKResponse::RoomName(roomid, name)) => {
                    let n = Some(name);
                    APPOP!(room_name_change, (roomid, n));
                }
                Ok(BKResponse::RoomTopic(roomid, topic)) => {
                    let t = Some(topic);
                    APPOP!(room_topic_change, (roomid, t));
                }
                Ok(BKResponse::NewRoomAvatar(roomid)) => {
                    APPOP!(new_room_avatar, (roomid));
                }
                Ok(BKResponse::RoomMemberEvent(ev)) => {
                    APPOP!(room_member_event, (ev));
                }
                Ok(BKResponse::Media(fname)) => {
                    Command::new("xdg-open")
                        .arg(&fname)
                        .spawn()
                        .expect("failed to execute process");
                }
                Ok(BKResponse::AttachedFile(msg)) => {
                    APPOP!(attached_file, (msg));
                }
                Ok(BKResponse::NewRoom(r, internal_id)) => {
                    let id = Some(internal_id);
                    APPOP!(new_room, (r, id));
                }
                Ok(BKResponse::AddedToFav(r, tofav)) => {
                    APPOP!(added_to_fav, (r, tofav));
                }
                Ok(BKResponse::UserSearch(users)) => {
                    APPOP!(user_search_finished, (users));
                }

                // errors
                Ok(BKResponse::AccountDestructionError(err)) => {
                    let error = i18n("Couldn’t delete the account");
                    error!("{:?}", err);
                    APPOP!(show_error_dialog, (error));
                }
                Ok(BKResponse::ChangePasswordError(err)) => {
                    let error = i18n("Couldn’t change the password");
                    error!("{:?}", err);
                    APPOP!(show_password_error_dialog, (error));
                }
                Ok(BKResponse::GetTokenEmailError(err)) => {
                    let error = i18n("Couldn’t add the email address.");
                    error!("{:?}", err);
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse::GetTokenPhoneError(err)) => {
                    let error = i18n("Couldn’t add the phone number.");
                    error!("{:?}", err);
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse::NewRoomError(err, internal_id)) => {
                    error!("{:?}", err);

                    let error = i18n("Can’t create the room, try again");
                    let panel = RoomPanel::NoRoom;
                    APPOP!(remove_room, (internal_id));
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                }
                Ok(BKResponse::JoinRoomError(err)) => {
                    error!("{:?}", err);
                    let error = format!("{}", i18n("Can’t join the room, try again."));
                    let panel = RoomPanel::NoRoom;
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                }
                Ok(BKResponse::LoginError(_)) => {
                    let error = i18n("Can’t login, try again");
                    let st = AppState::Login;
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (st));
                }
                Ok(BKResponse::AttachFileError(err)) => {
                    error!("attaching {:?}: retrying send", err);
                    APPOP!(retry_send);
                }
                Ok(BKResponse::SendMsgError(err)) => match err {
                    Error::SendMsgError(txid) => {
                        error!("sending {}: retrying send", txid);
                        APPOP!(retry_send);
                    }
                    _ => {
                        let error = i18n("Error sending message");
                        APPOP!(show_error, (error));
                    }
                },
                Ok(BKResponse::SendMsgRedactionError(_)) => {
                    let error = i18n("Error deleting message");
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::DirectoryError(_)) => {
                    let error = i18n("Error searching for rooms");
                    APPOP!(reset_directory_state);
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::SyncError(err)) => {
                    error!("SYNC Error: {:?}", err);
                    APPOP!(sync_error);
                }
                Ok(err) => {
                    error!("Query error: {:?}", err);
                }
            };
        }
    });
}
