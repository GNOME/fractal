use app::App;

use appop::MsgPos;
use appop::RoomPanel;
use appop::AppState;

use std::thread;
use std::sync::mpsc::Receiver;
use glib;

use types::Message;
use types::Room;
use types::Member;
use types::Sticker;
use types::StickerGroup;


#[derive(Debug)]
pub enum InternalCommand {
    AddRoomMessage(Message, MsgPos, Option<Message>, bool, bool),
    SetPanel(RoomPanel),
    SetView(AppState),
    NotifyClicked(Message),
    SelectRoom(Room),
    LoadMore,
    LoadMoreNormal,
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
    thread::spawn(move || {
        loop {
            let recv = rx.recv();
            match recv {
                Ok(InternalCommand::AddRoomMessage(msg, pos, prev, force_full, first_new)) => {
                    APPOP!(add_room_message, (msg));
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
                Ok(InternalCommand::LoadMore) => {
                    APPOP!(load_more_messages);
                }
                Ok(InternalCommand::LoadMoreNormal) => {
                    APPOP!(load_more_normal);
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
        }
    });
}
