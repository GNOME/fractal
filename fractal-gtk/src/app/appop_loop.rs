use app::App;

use appop::MsgPos;
use appop::LastViewed;
use appop::RoomPanel;

use std::thread;
use std::sync::mpsc::Receiver;
use glib;

use types::Message;
use types::Room;
use types::Member;
use types::Sticker;


#[derive(Debug)]
pub enum InternalCommand {
    AddRoomMessage(Message, MsgPos, Option<Message>, bool, LastViewed),
    SetPanel(RoomPanel),
    NotifyClicked(Message),
    SelectRoom(Room),
    LoadMoreNormal,
    RemoveInv(String),
    SendSticker(Sticker),

    ToInvite(Member),
    RmInvite(String),
}


pub fn appop_loop(rx: Receiver<InternalCommand>) {
    thread::spawn(move || {
        loop {
            let recv = rx.recv();
            match recv {
                Ok(InternalCommand::AddRoomMessage(msg, pos, prev, force_full, last)) => {
                    APPOP!(add_room_message, (msg, pos, prev, force_full, last));
                }
                Ok(InternalCommand::ToInvite(member)) => {
                    APPOP!(add_to_invite, (member));
                }
                Ok(InternalCommand::RmInvite(uid)) => {
                    APPOP!(rm_from_invite, (uid));
                }
                Ok(InternalCommand::SetPanel(st)) => {
                    APPOP!(room_panel, (st));
                }
                Ok(InternalCommand::NotifyClicked(msg)) => {
                    APPOP!(notification_cliked, (msg));
                }
                Ok(InternalCommand::SelectRoom(r)) => {
                    let id = r.id;
                    APPOP!(set_active_room_by_id, (id));
                }
                Ok(InternalCommand::LoadMoreNormal) => {
                    APPOP!(load_more_normal);
                }
                Ok(InternalCommand::RemoveInv(rid)) => {
                    APPOP!(remove_inv, (rid));
                }
                Ok(InternalCommand::SendSticker(sticker)) => {
                    APPOP!(send_sticker, (sticker));
                }
                Err(_) => {
                    break;
                }
            };
        }
    });
}
