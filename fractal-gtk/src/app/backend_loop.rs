extern crate gettextrs;

use self::gettextrs::gettext;

use app::App;

use appop::RoomPanel;
use appop::AppState;

use std::thread;
use std::sync::mpsc::Receiver;
use std::process::Command;
use glib;

use backend::BKResponse;

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
                Err(RecvError) => { break; }
                Ok(BKResponse::ShutDown) => { shutting_down = true; }
                Ok(BKResponse::Token(uid, tk)) => {
                    APPOP!(bk_login, (uid, tk));

                    // after login
                    APPOP!(sync);
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
                Ok(BKResponse::GetTokenEmail(sid)) => {
                    let sid = Some(sid);
                    APPOP!(get_token_email, (sid));
                }
                Ok(BKResponse::GetTokenPhone(sid)) => {
                    let sid = Some(sid);
                    APPOP!(get_token_phone, (sid));
                }
                Ok(BKResponse:: GetTokenEmailUsed) => {
                    let error = gettext("Email is already in use");
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse:: GetTokenPhoneUsed) => {
                    let error = gettext("Phone number is already in use");
                    APPOP!(show_three_pid_error_dialog, (error));
                }
                Ok(BKResponse:: SubmitPhoneToken(sid)) => {
                    APPOP!(valid_phone_token, (sid));
                }
                Ok(BKResponse:: AddThreePID(list)) => {
                    let l = Some(list);
                    APPOP!(added_three_pid, (l));
                }
                Ok(BKResponse::DeleteThreePID) => {
                    APPOP!(get_three_pid);
                }
                Ok(BKResponse::SetUserName(username)) => {
                    let u = Some(username);
                    APPOP!(set_username, (u));
                }
                Ok(BKResponse::Avatar(path)) => {
                    let av = Some(path);
                    APPOP!(set_avatar, (av));
                }
                Ok(BKResponse::SetUserAvatar(path)) => {
                    let av = Some(path);
                    APPOP!(set_avatar, (av));
                }
                Ok(BKResponse::Sync(since)) => {
                    println!("SYNC");
                    let s = Some(since);
                    APPOP!(synced, (s));
                }
                Ok(BKResponse::Rooms(rooms, default)) => {
                    APPOP!(update_rooms, (rooms, default));
                }
                Ok(BKResponse::NewRooms(rooms)) => {
                    APPOP!(new_rooms, (rooms));
                }
                Ok(BKResponse::RoomDetail(room, key, value)) => {
                    let v = Some(value);
                    APPOP!(set_room_detail, (room, key, v));
                }
                Ok(BKResponse::RoomAvatar(room, avatar)) => {
                    let a = Some(avatar);
                    APPOP!(set_room_avatar, (room, a));
                }
                Ok(BKResponse::RoomMembers(room, members)) => {
                    APPOP!(set_room_members, (room, members));
                }
                Ok(BKResponse::RoomMessages(msgs)) => {
                    let init = false;
                    APPOP!(show_room_messages, (msgs, init));
                }
                Ok(BKResponse::RoomMessagesInit(msgs)) => {
                    let init = true;
                    APPOP!(show_room_messages, (msgs, init));
                }
                Ok(BKResponse::RoomMessagesTo(msgs)) => {
                    APPOP!(show_room_messages_top, (msgs));
                }
                Ok(BKResponse::SendMsg) => {
                    APPOP!(sync);
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
                Ok(BKResponse::LeaveRoom) => { }
                Ok(BKResponse::SetRoomName) => { }
                Ok(BKResponse::SetRoomTopic) => { }
                Ok(BKResponse::SetRoomAvatar) => { }
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
                    APPOP!(add_tmp_room_message, (msg));
                }
                Ok(BKResponse::SearchEnd) => {
                    APPOP!(search_end);
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
                Ok(BKResponse::Stickers(stickers)) => {
                    APPOP!(stickers_loaded, (stickers));
                }

                // errors
                Ok(BKResponse::GetTokenEmailError(err)) => {
                    let error = gettext("Couldn't add the email address.");
                    println!("ERROR: {:?}", err);
                    APPOP!(show_three_pid_error_dialog, (error));
                },
                Ok(BKResponse::NewRoomError(err, internal_id)) => {
                    println!("ERROR: {:?}", err);

                    let error = gettext("Can’t create the room, try again");
                    let panel = RoomPanel::NoRoom;
                    APPOP!(remove_room, (internal_id));
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                },
                Ok(BKResponse::JoinRoomError(err)) => {
                    println!("ERROR: {:?}", err);
                    let error = format!("{}", gettext("Can’t join the room, try again."));
                    let panel = RoomPanel::NoRoom;
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                },
                Ok(BKResponse::LoginError(_)) => {
                    let error = gettext("Can’t login, try again");
                    let st = AppState::Login;
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (st));
                },
                Ok(BKResponse::SendMsgError(_)) => {
                    let error = gettext("Error sending message");
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::DirectoryError(_)) => {
                    let error = gettext("Error searching for rooms");
                    APPOP!(reset_directory_state);
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::SyncError(err)) => {
                    println!("SYNC Error: {:?}", err);
                    APPOP!(sync_error);
                }
                Ok(err) => {
                    println!("Query error: {:?}", err);
                }
            };
        }
    });
}
