use crate::app::App;
use crate::i18n::i18n;
use log::{error, info};
use regex::Regex;

use crate::actions::{activate_action, AppState};

use glib;
use std::borrow::Cow;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::thread;

use crate::backend::BKResponse;
use fractal_api::error::Error;

pub fn backend_loop(rx: Receiver<BKResponse>) {
    thread::spawn(move || {
        while let Ok(recv) = rx.recv() {
            match recv {
                BKResponse::ShutDown => {
                    break;
                }
                BKResponse::Token(uid, tk, dev, server_url, id_url) => {
                    APPOP!(bk_login, (uid, tk, dev, server_url, id_url));
                }
                BKResponse::Logout(Ok(_)) => {
                    APPOP!(bk_logout);
                }
                BKResponse::Name(Ok(username)) => {
                    APPOP!(set_username, (username));
                }
                BKResponse::GetThreePID(Ok(list)) => {
                    let l = Some(list);
                    APPOP!(set_three_pid, (l));
                }
                BKResponse::GetTokenEmail(Ok((sid, secret))) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_email, (sid, secret));
                }
                BKResponse::GetTokenPhone(Ok((sid, secret))) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_phone, (sid, secret));
                }
                BKResponse::GetTokenEmail(Err(Error::TokenUsed)) => {
                    let error = i18n("Email is already in use");
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::GetTokenEmail(Err(Error::Denied)) => {
                    let error = i18n("Please enter a valid email address.");
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::GetTokenPhone(Err(Error::TokenUsed)) => {
                    let error = i18n("Phone number is already in use");
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::GetTokenPhone(Err(Error::Denied)) => {
                    let error = i18n(
                        "Please enter your phone number in the format: \n + your country code and your phone number.",
                    );
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::SubmitPhoneToken(Ok((sid, secret))) => {
                    let secret = Some(secret);
                    APPOP!(valid_phone_token, (sid, secret));
                }
                BKResponse::AddThreePID(Ok(_)) => {
                    APPOP!(added_three_pid);
                }
                BKResponse::DeleteThreePID(Ok(_)) => {
                    APPOP!(get_three_pid);
                }
                BKResponse::ChangePassword(Ok(_)) => {
                    APPOP!(password_changed);
                }
                BKResponse::SetUserName(Ok(username)) => {
                    let u = Some(username);
                    APPOP!(show_new_username, (u));
                }
                BKResponse::AccountDestruction(Ok(_)) => {
                    APPOP!(account_destruction_logoff);
                }
                BKResponse::Avatar(Ok(path)) => {
                    let av = Some(path);
                    APPOP!(set_avatar, (av));
                }
                BKResponse::SetUserAvatar(Ok(path)) => {
                    let av = Some(path);
                    APPOP!(show_new_avatar, (av));
                }
                BKResponse::Sync(Ok(since)) => {
                    info!("SYNC");
                    let s = Some(since);
                    APPOP!(synced, (s));
                }
                BKResponse::Rooms(Ok((rooms, default))) => {
                    let clear_room_list = true;
                    APPOP!(set_rooms, (rooms, clear_room_list));
                    // Open the newly joined room
                    if let Some(room) = default {
                        let room_id = room.id;
                        APPOP!(set_active_room_by_id, (room_id));
                    }
                }
                BKResponse::UpdateRooms(Ok(rooms)) => {
                    let clear_room_list = false;
                    APPOP!(set_rooms, (rooms, clear_room_list));
                }
                BKResponse::RoomDetail(Ok((room, key, value))) => {
                    let v = Some(value);
                    APPOP!(set_room_detail, (room, key, v));
                }
                BKResponse::RoomAvatar(Ok((room, avatar))) => {
                    APPOP!(set_room_avatar, (room, avatar));
                }
                BKResponse::RoomMembers(Ok((room, members))) => {
                    APPOP!(set_room_members, (room, members));
                }
                BKResponse::RoomMessages(Ok(msgs)) => {
                    APPOP!(show_room_messages, (msgs));
                }
                BKResponse::RoomMessagesTo(Ok((msgs, room, prev_batch))) => {
                    APPOP!(show_room_messages_top, (msgs, room, prev_batch));
                }
                BKResponse::SentMsg(Ok((txid, evid))) => {
                    APPOP!(msg_sent, (txid, evid));
                    let initial = false;
                    APPOP!(sync, (initial));
                }
                BKResponse::DirectoryProtocols(Ok(protocols)) => {
                    APPOP!(set_protocols, (protocols));
                }
                BKResponse::DirectorySearch(Ok(rooms)) => {
                    APPOP!(append_directory_rooms, (rooms));
                }

                BKResponse::JoinRoom(Ok(_)) => {
                    APPOP!(reload_rooms);
                }
                BKResponse::LeaveRoom(Ok(_)) => {}
                BKResponse::SetRoomName(Ok(_)) => {
                    APPOP!(show_new_room_name);
                }
                BKResponse::SetRoomTopic(Ok(_)) => {
                    APPOP!(show_new_room_topic);
                }
                BKResponse::SetRoomAvatar(Ok(_)) => {
                    APPOP!(show_new_room_avatar);
                }
                BKResponse::RemoveMessage(Ok((room, msg))) => {
                    APPOP!(remove_message, (room, msg));
                }
                BKResponse::MarkedAsRead(Ok((r, _))) => {
                    APPOP!(clear_room_notifications, (r));
                }
                BKResponse::RoomNotifications(r, n, h) => {
                    APPOP!(set_room_notifications, (r, n, h));
                }

                BKResponse::RoomName(roomid, name) => {
                    let n = Some(name);
                    APPOP!(room_name_change, (roomid, n));
                }
                BKResponse::RoomTopic(roomid, topic) => {
                    let t = Some(topic);
                    APPOP!(room_topic_change, (roomid, t));
                }
                BKResponse::NewRoomAvatar(roomid) => {
                    APPOP!(new_room_avatar, (roomid));
                }
                BKResponse::RoomMemberEvent(ev) => {
                    APPOP!(room_member_event, (ev));
                }
                BKResponse::Media(Ok(fname)) => {
                    Command::new("xdg-open")
                        .arg(&fname)
                        .spawn()
                        .expect("failed to execute process");
                }
                BKResponse::AttachedFile(Ok(msg)) => {
                    APPOP!(attached_file, (msg));
                }
                BKResponse::NewRoom(Ok(r), internal_id) => {
                    let id = Some(internal_id);
                    APPOP!(new_room, (r, id));
                }
                BKResponse::AddedToFav(Ok((r, tofav))) => {
                    APPOP!(added_to_fav, (r, tofav));
                }
                BKResponse::UserSearch(Ok(users)) => {
                    APPOP!(user_search_finished, (users));
                }

                // errors
                BKResponse::AccountDestruction(Err(err)) => {
                    let error = i18n("Couldn’t delete the account");
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::ChangePassword(Err(err)) => {
                    let error = i18n("Couldn’t change the password");
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(show_password_error_dialog, (error));
                }
                BKResponse::GetThreePID(Err(_)) => {
                    let error = i18n("Sorry, account settings can’t be loaded.");
                    APPOP!(show_load_settings_error_dialog, (error));
                    let ctx = glib::MainContext::default();
                    ctx.invoke(move || {
                        activate_action("app", "back");
                    })
                }
                BKResponse::GetTokenEmail(Err(err)) => {
                    let error = i18n("Couldn’t add the email address.");
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::GetTokenPhone(Err(err)) => {
                    let error = i18n("Couldn’t add the phone number.");
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(show_error_dialog_in_settings, (error));
                }
                BKResponse::NewRoom(Err(err), internal_id) => {
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );

                    let error = i18n("Can’t create the room, try again");
                    let state = AppState::NoRoom;
                    APPOP!(remove_room, (internal_id));
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (state));
                }
                BKResponse::JoinRoom(Err(err)) => {
                    error!(
                        "{}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );

                    let error = format!("{}", i18n("Can’t join the room, try again."));
                    let state = AppState::NoRoom;
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (state));
                }
                BKResponse::ChangeLanguage(Err(err)) => {
                    error!(
                        "Error forming url to set room language: {}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                }
                BKResponse::LoginError(_) => {
                    let error = i18n("Can’t login, try again");
                    let st = AppState::Login;
                    APPOP!(show_error, (error));
                    APPOP!(logout);
                    APPOP!(set_state, (st));
                }
                BKResponse::AttachedFile(Err(err)) => {
                    error!(
                        "attaching {}: retrying send",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(retry_send);
                }
                BKResponse::SentMsg(Err(err)) => match err {
                    Error::SendMsgError(txid) => {
                        error!("sending {}: retrying send", txid);
                        APPOP!(retry_send);
                    }
                    _ => {
                        let error = i18n("Error sending message");
                        APPOP!(show_error, (error));
                    }
                },
                BKResponse::SentMsgRedaction(Err(_)) => {
                    let error = i18n("Error deleting message");
                    APPOP!(show_error, (error));
                }
                BKResponse::DirectoryProtocols(Err(_)) | BKResponse::DirectorySearch(Err(_)) => {
                    let error = i18n("Error searching for rooms");
                    APPOP!(reset_directory_state);
                    APPOP!(show_error, (error));
                }
                BKResponse::Sync(Err(err)) => {
                    error!(
                        "SYNC Error: {}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                    APPOP!(sync_error);
                }
                err => {
                    error!(
                        "Query error: {}",
                        remove_matrix_access_token_if_present(&format!("{:?}", err))
                    );
                }
            };
        }
    });
}

/// This function removes the value of the `access_token` query from a URL used for accessing the Matrix API.
/// The primary use case is the removing of sensitive information for logging.
/// Specifically, the URL is expected to be contained within quotes and the token is replaced wiht `<redacted>`.
fn remove_matrix_access_token_if_present<'a>(message: &'a str) -> Cow<'a, str> {
    let re = Regex::new(
        r#""((http)|(https))://([^"]+)/_matrix/([^"]+)\?access_token=(?P<token>[^&"]+)([^"]*)""#,
    )
    .unwrap();
    if let Some(cap) = re.captures(message) {
        let captured_token = cap.name("token").unwrap().as_str();
        Cow::Owned(message.replace(captured_token, "<redacted>"))
    } else {
        message.into()
    }
}
