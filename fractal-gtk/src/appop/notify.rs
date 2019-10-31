use gio::ApplicationExt;
use gio::FileExt;
use gio::Notification;
use gtk;
use gtk::prelude::*;
use log::info;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use crate::i18n::i18n;

use crate::appop::AppOp;
use crate::backend::BKCommand;

use crate::widgets::ErrorDialog;

impl AppOp {
    pub fn inapp_notify(&self, msg: &str) {
        let inapp: gtk::Revealer = self
            .ui
            .builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        let label: gtk::Label = self
            .ui
            .builder
            .get_object("inapp_label")
            .expect("Can't find inapp_label in ui file.");
        label.set_text(msg);
        inapp.set_reveal_child(true);
    }

    pub fn hide_inapp_notify(&self) {
        let inapp: gtk::Revealer = self
            .ui
            .builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        inapp.set_reveal_child(false);
    }

    pub fn notify(&self, app: gtk::Application, room_id: &str, id: &str) -> Option<()> {
        let msg = self.get_message_by_id(room_id, id)?;
        let r = self.rooms.get(room_id)?;
        let body = dirty_truncate(&msg.body, 80);

        let title = if r.direct {
            i18n(" (direct message)")
        } else {
            if let Some(name) = r.name.clone() {
                format!(" ({})", name)
            } else {
                String::new()
            }
        };

        let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
        let _ = self.backend.send(BKCommand::GetUserInfoAsync(
            self.server_url.clone(),
            msg.sender.clone(),
            Some(tx),
        ));

        let room_id = room_id.to_string();
        let id = id.to_string();
        let app_weak = app.downgrade();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Ok((name, avatar_path)) => {
                let title = format!("{}{}", name, title);
                let app = upgrade_weak!(app_weak, gtk::Continue(false));
                let n = create_notification(&room_id, &title, &body, &avatar_path);
                app.send_notification(Some(id.as_str()), &n);
                gtk::Continue(false)
            }
        });

        None
    }

    pub fn show_error(&self, msg: String) {
        ErrorDialog::new(false, &msg);
    }
}

fn dirty_truncate(s: &str, num_chars: usize) -> &str {
    let l = s.len();

    println!("len: {}", l);
    if l <= num_chars {
        s
    } else {
        for (idx, ch) in s.char_indices() {
            println!("idx {} ch {}", idx, ch);
        }
        if let Some((idx, _ch)) = s.char_indices().find(|(idx, _ch)| *idx >= num_chars) {
            s.get(0..idx).unwrap()
        } else {
            s
        }
    }
}

fn create_notification(room_id: &str, title: &str, body: &str, avatar: &str) -> Notification {
    let notification = Notification::new(title);
    notification.set_body(Some(body));
    notification.set_priority(gio::NotificationPriority::High);
    info!("Creating notification with avatar: {}", avatar);
    let file = gio::File::new_for_path(avatar);
    let _ = file.load_bytes(gio::NONE_CANCELLABLE).map(|(b, _)| {
        let avatar = gio::BytesIcon::new(&b);
        notification.set_icon(&avatar);
    });
    let data = glib::Variant::from(room_id);
    notification.set_default_action_and_target_value("app.open-room", Some(&data));
    notification
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_truncate_works() {
        assert_eq!(dirty_truncate("hello", 80), "hello");

        assert_eq!(
            dirty_truncate(
                "0123456789012345678901234567890123456789012345678901234567890123456789012345678áéíóú",
                80
            ),
            "0123456789012345678901234567890123456789012345678901234567890123456789012345678á"
        );
        
        // len 82, max index 79 for the ideograph
        assert_eq!(
            dirty_truncate(
                "0123456789012345678901234567890123456789012345678901234567890123456789012345678㈨",
                80
            ),
            "0123456789012345678901234567890123456789012345678901234567890123456789012345678㈨"
        );
    }
}
