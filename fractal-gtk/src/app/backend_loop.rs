use crate::app::App;
use crate::i18n::i18n;
use log::{error, info};

use crate::actions::AppState;

use glib;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::thread;

use crate::backend::BKResponse;
use fractal_api::error::Error;

use std::sync::mpsc::RecvError;

pub fn backend_loop(rx: Receiver<BKResponse>) {
    thread::spawn(move || {
        loop {
            let recv = rx.recv();

            match recv {
                Err(RecvError) => {
                    break;
                }
                Ok(BKResponse::ShutDown) => {
                    break;
                }
                Ok(BKResponse::Token(uid, tk, dev)) => {
                    APPOP!(bk_login, (uid, tk, dev));
                }
                Ok(BKResponse::Logout(Ok(_))) => {
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
                Ok(BKResponse::Sync(Ok(since))) => {
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
                Ok(BKResponse::UpdateRooms(rooms)) => {
                    let clear_room_list = false;
                    APPOP!(set_rooms, (rooms, clear_room_list));
                }
                Ok(BKResponse::RoomDetail(Ok((room, key, value)))) => {
                    let v = Some(value);
                    APPOP!(set_room_detail, (room, key, v));
                }
                Ok(BKResponse::RoomAvatar(Ok((room, avatar)))) => {
                    APPOP!(set_room_avatar, (room, avatar));
                }
                Ok(BKResponse::RoomMembers(Ok((room, members)))) => {
                    APPOP!(set_room_members, (room, members));
                }
                Ok(BKResponse::RoomMessages(msgs)) => {
                    APPOP!(show_room_messages, (msgs));
                }
                Ok(BKResponse::RoomMessagesTo(Ok((msgs, room, prev_batch)))) => {
                    APPOP!(show_room_messages_top, (msgs, room, prev_batch));
                }
                Ok(BKResponse::SentMsg(Ok((txid, evid)))) => {
                    APPOP!(msg_sent, (txid, evid));
                    let initial = false;
                    APPOP!(sync, (initial));
                }
                Ok(BKResponse::DirectoryProtocols(Ok(protocols))) => {
                    APPOP!(set_protocols, (protocols));
                }
                Ok(BKResponse::DirectorySearch(Ok(rooms))) => {
                    APPOP!(append_directory_rooms, (rooms));
                }

                Ok(BKResponse::JoinRoom(Ok(_))) => {
                    APPOP!(reload_rooms);
                }
                Ok(BKResponse::LeaveRoom(Ok(_))) => {}
                Ok(BKResponse::SetRoomName(Ok(_))) => {
                    APPOP!(show_new_room_name);
                }
                Ok(BKResponse::SetRoomTopic(Ok(_))) => {
                    APPOP!(show_new_room_topic);
                }
                Ok(BKResponse::SetRoomAvatar(Ok(_))) => {
                    APPOP!(show_new_room_avatar);
                }
                Ok(BKResponse::MarkedAsRead(Ok((r, _)))) => {
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
                Ok(BKResponse::Media(Ok(fname))) => {
                    Command::new("xdg-open")
                        .arg(&fname)
                        .spawn()
                        .expect("failed to execute process");
                }
                Ok(BKResponse::AttachedFile(Ok(msg))) => {
                    APPOP!(attached_file, (msg));
                }
                Ok(BKResponse::NewRoom(Ok(r), internal_id)) => {
                    let id = Some(internal_id);
                    APPOP!(new_room, (r, id));
                }
                Ok(BKResponse::AddedToFav(Ok((r, tofav)))) => {
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
                Ok(BKResponse::NewRoom(Err(err), internal_id)) => {
                    error!("{:?}", err);

                    let error = i18n("Can’t create the room, try again");
                    let state = AppState::NoRoom;
                    APPOP!(remove_room, (internal_id));
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (state));
                }
                Ok(BKResponse::JoinRoom(Err(err))) => {
                    error!("{:?}", err);
                    let error = format!("{}", i18n("Can’t join the room, try again."));
                    let state = AppState::NoRoom;
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (state));
                }
                Ok(BKResponse::LoginError(_)) => {
                    let error = i18n("Can’t login, try again");
                    let st = AppState::Login;
                    APPOP!(show_error, (error));
                    APPOP!(logout);
                    APPOP!(set_state, (st));
                }
                Ok(BKResponse::AttachedFile(Err(err))) => {
                    error!("attaching {:?}: retrying send", err);
                    APPOP!(retry_send);
                }
                Ok(BKResponse::SentMsg(Err(err))) => match err {
                    Error::SendMsgError(txid) => {
                        error!("sending {}: retrying send", txid);
                        APPOP!(retry_send);
                    }
                    _ => {
                        let error = i18n("Error sending message");
                        APPOP!(show_error, (error));
                    }
                },
                Ok(BKResponse::SentMsgRedaction(Err(_))) => {
                    let error = i18n("Error deleting message");
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::DirectoryProtocols(Err(_)))
                | Ok(BKResponse::DirectorySearch(Err(_))) => {
                    let error = i18n("Error searching for rooms");
                    APPOP!(reset_directory_state);
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::Sync(Err(err))) => {
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
