use fractal_api::util::ResultExpectLog;
use log::info;
use std::thread;

use crate::i18n::i18n;

use crate::app::App;
use crate::appop::AppOp;

use crate::backend::{
    sync::{self, RoomElement, SyncRet},
    BKCommand, BKResponse,
};

impl AppOp {
    pub fn initial_sync(&self, show: bool) {
        if show {
            self.inapp_notify(&i18n("Syncing, this could take a while"));
        } else {
            self.hide_inapp_notify();
        }
    }

    pub fn sync(&mut self, initial: bool, number_tries: u64) {
        if let (Some(login_data), false) = (self.login_data.clone(), self.syncing) {
            self.syncing = true;
            // for the initial sync we set the since to None to avoid long syncing
            // the since can be a very old value and following the spec we should
            // do the initial sync without a since:
            // https://matrix.org/docs/spec/client_server/latest.html#syncing
            let join_to_room = self.join_to_room.clone();
            let since = self.since.clone().filter(|_| !initial);
            let tx = self.backend.clone();
            thread::spawn(move || {
                match sync::sync(
                    login_data.server_url,
                    login_data.access_token,
                    login_data.uid,
                    join_to_room,
                    since,
                    initial,
                    number_tries,
                ) {
                    Ok(SyncRet::NoSince { rooms, next_batch }) => {
                        match rooms {
                            Ok((rooms, default)) => {
                                let clear_room_list = true;
                                APPOP!(set_rooms, (rooms, clear_room_list));
                                // Open the newly joined room
                                let jtr = default.as_ref().map(|r| r.id.clone());
                                APPOP!(set_join_to_room, (jtr));
                                if let Some(room) = default {
                                    let room_id = room.id;
                                    APPOP!(set_active_room_by_id, (room_id));
                                }
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(BKResponse::RoomsError(err)))
                                    .expect_log("Connection closed");
                            }
                        };

                        info!("SYNC");
                        let s = Some(next_batch);
                        APPOP!(synced, (s));
                    }
                    Ok(SyncRet::WithSince {
                        update_rooms,
                        room_messages,
                        room_notifications,
                        update_rooms_2,
                        other,
                        next_batch,
                    }) => {
                        match update_rooms {
                            Ok(rooms) => {
                                let clear_room_list = false;
                                APPOP!(set_rooms, (rooms, clear_room_list));
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(BKResponse::UpdateRoomsError(
                                    err,
                                )))
                                .expect_log("Connection closed");
                            }
                        }

                        match room_messages {
                            Ok(msgs) => {
                                APPOP!(show_room_messages, (msgs));
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(BKResponse::RoomMessagesError(
                                    err,
                                )))
                                .expect_log("Connection closed");
                            }
                        }

                        match update_rooms_2 {
                            Ok(rooms) => {
                                let clear_room_list = false;
                                APPOP!(set_rooms, (rooms, clear_room_list));
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(BKResponse::UpdateRoomsError(
                                    err,
                                )))
                                .expect_log("Connection closed");
                            }
                        }

                        for (room_id, unread_notifications) in room_notifications {
                            let r = room_id;
                            let n = unread_notifications.notification_count;
                            let h = unread_notifications.highlight_count;
                            APPOP!(set_room_notifications, (r, n, h));
                        }

                        match other {
                            Ok(other) => {
                                for room_element in other {
                                    match room_element {
                                        RoomElement::Name(room_id, name) => {
                                            let n = Some(name);
                                            APPOP!(room_name_change, (room_id, n));
                                        }
                                        RoomElement::Topic(room_id, topic) => {
                                            let t = Some(topic);
                                            APPOP!(room_topic_change, (room_id, t));
                                        }
                                        RoomElement::NewAvatar(room_id) => {
                                            APPOP!(new_room_avatar, (room_id));
                                        }
                                        RoomElement::MemberEvent(event) => {
                                            APPOP!(room_member_event, (event));
                                        }
                                        RoomElement::RemoveMessage(room_id, msg_id) => {
                                            APPOP!(remove_message, (room_id, msg_id));
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(BKResponse::RoomElementError(
                                    err,
                                )))
                                .expect_log("Connection closed");
                            }
                        }

                        info!("SYNC");
                        let s = Some(next_batch);
                        APPOP!(synced, (s));
                    }
                    Err((err, n_tries)) => {
                        tx.send(BKCommand::SendBKResponse(BKResponse::SyncError(
                            err, n_tries,
                        )))
                        .expect_log("Connection closed");
                    }
                }
            });
        }
    }

    pub fn synced(&mut self, since: Option<String>) {
        self.syncing = false;
        self.since = since;
        self.sync(false, 0);
        self.initial_sync(false);
    }

    pub fn sync_error(&mut self, number_tries: u64) {
        self.syncing = false;
        self.sync(false, number_tries);
    }
}
