use app::App;

use appop::AppState;

use glib;
use std::sync::mpsc::Receiver;
use std::thread;

use types::Member;
use types::Message;
use types::Room;
use types::Sticker;
use types::StickerGroup;

#[derive(Debug)]
pub enum InternalCommand {
    SetView(AppState),
    NotifyClicked(Message),
    SelectRoom(Room),
    RemoveInv(String),
    AppendTmpMessages,
    ForceDequeueMessage,
    AttachMessage(String),
    #[allow(dead_code)]
    SendSticker(Sticker),
    #[allow(dead_code)]
    PurchaseSticker(StickerGroup),

    ToInvite(Member),
    RmInvite(String),
}

pub fn appop_loop(rx: Receiver<InternalCommand>) {
    thread::spawn(move || loop {
        let recv = rx.recv();
        match recv {
            Ok(InternalCommand::ToInvite(member)) => {
                APPOP!(add_to_invite, (member));
            }
            Ok(InternalCommand::RmInvite(uid)) => {
                APPOP!(rm_from_invite, (uid));
            }
            Ok(InternalCommand::SetView(view)) => {
                APPOP!(set_state, (view));
            }
            Ok(InternalCommand::NotifyClicked(msg)) => {
                APPOP!(notification_cliked, (msg));
            }
            Ok(InternalCommand::SelectRoom(r)) => {
                let id = r.id;
                APPOP!(set_active_room_by_id, (id));
            }
            Ok(InternalCommand::RemoveInv(rid)) => {
                APPOP!(remove_inv, (rid));
            }
            Ok(InternalCommand::AppendTmpMessages) => {
                APPOP!(append_tmp_msgs);
            }
            Ok(InternalCommand::ForceDequeueMessage) => {
                APPOP!(force_dequeue_message);
            }
            Ok(InternalCommand::AttachMessage(file)) => {
                APPOP!(attach_message, (file));
            }
            Ok(InternalCommand::SendSticker(sticker)) => {
                APPOP!(send_sticker, (sticker));
            }
            Ok(InternalCommand::PurchaseSticker(group)) => {
                APPOP!(purchase_sticker, (group));
            }
            Err(_) => {
                break;
            }
        };
    });
}
