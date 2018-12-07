use app::App;

use appop::AppState;

use glib;
use std::sync::mpsc::Receiver;
use std::thread;

use types::Member;
use types::Sticker;
use types::StickerGroup;

#[derive(Debug)]
pub enum InternalCommand {
    SetView(AppState),
    LoadMore,
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
            Ok(InternalCommand::LoadMore) => {
                APPOP!(load_more_messages);
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
