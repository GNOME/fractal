extern crate notify_rust;
extern crate gtk;

use self::gtk::prelude::*;
use self::notify_rust::Notification;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;

use appop::AppOp;
use app::InternalCommand;
use backend::BKCommand;

use types::Message;
use rayon;

impl AppOp {
    pub fn inapp_notify(&self, msg: &str) {
        let inapp: gtk::Revealer = self.ui.builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        let label: gtk::Label = self.ui.builder
            .get_object("inapp_label")
            .expect("Can't find inapp_label in ui file.");
        label.set_text(msg);
        inapp.set_reveal_child(true);
    }

    pub fn hide_inapp_notify(&self) {
        let inapp: gtk::Revealer = self.ui.builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        inapp.set_reveal_child(false);
    }

    pub fn notify(&self, msg: &Message) {
        let roomname = match self.rooms.get(&msg.room) {
            Some(r) => r.name.clone().unwrap_or_default(),
            None => msg.room.clone(),
        };

        let mut body = msg.body.clone();
        body.truncate(80);

        let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
        self.backend.send(BKCommand::GetUserInfoAsync(msg.sender.clone(), Some(tx))).unwrap();
        let bk = self.internal.clone();
        let m = msg.clone();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Ok((name, avatar)) => {
                let summary = format!("@{} / {}", name, roomname);

                let bk = bk.clone();
                let m = m.clone();
                let body = body.clone();
                let summary = summary.clone();
                let avatar = avatar.clone();
                rayon::spawn(move || {
                    let mut notification = Notification::new();
                    notification.summary(&summary);
                    notification.body(&body);
                    notification.icon(&avatar);
                    notification.action("default", "default");

                    if let Ok(n) = notification.show() {
                        #[cfg(all(unix, not(target_os = "macos")))]
                        n.wait_for_action({|action|
                            match action {
                                "default" => {
                                    bk.send(InternalCommand::NotifyClicked(m)).unwrap();
                                },
                                _ => ()
                            }
                        });
                    }
                });

                gtk::Continue(false)
            }
        });
    }

    pub fn show_error(&self, msg: String) {
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        let dialog = gtk::MessageDialog::new(Some(&window),
                                             gtk::DialogFlags::MODAL,
                                             gtk::MessageType::Warning,
                                             gtk::ButtonsType::Ok,
                                             &msg);
        dialog.show();
        dialog.connect_response(move |d, _| { d.destroy(); });
    }

    pub fn notification_cliked(&mut self, msg: Message) {
        self.activate();
        let mut room = None;
        if let Some(r) = self.rooms.get(&msg.room) {
            room = Some(r.clone());
        }

        if let Some(r) = room {
            self.set_active_room_by_id(r.id.clone());
        }
    }
}
