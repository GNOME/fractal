use gdk;
use gtk;
use gtk::prelude::*;

use std::sync::{Arc, Mutex};
use glib;

use app::App;

impl App {
    pub fn connect_invite_dialog(&self) {
        let op = self.op.clone();
        let dialog = self.ui.builder
            .get_object::<gtk::MessageDialog>("invite_dialog")
            .expect("Can't find invite_dialog in ui file.");
        let accept = self.ui.builder
            .get_object::<gtk::Button>("invite_accept")
            .expect("Can't find invite_accept in ui file.");
        let reject = self.ui.builder
            .get_object::<gtk::Button>("invite_reject")
            .expect("Can't find invite_reject in ui file.");

        reject.connect_clicked(clone!(dialog, op => move |_| {
            op.lock().unwrap().accept_inv(false);
            dialog.hide();
        }));
        dialog.connect_delete_event(clone!(dialog, op => move |_, _| {
            op.lock().unwrap().accept_inv(false);
            dialog.hide();
            glib::signal::Inhibit(true)
        }));

        accept.connect_clicked(clone!(dialog, op => move |_| {
            op.lock().unwrap().accept_inv(true);
            dialog.hide();
        }));
    }

    pub fn connect_invite_user(&self) {
        let op = &self.op;

        let cancel = self.ui.builder
            .get_object::<gtk::Button>("cancel_invite")
            .expect("Can't find cancel_invite in ui file.");
        let invite = self.ui.builder
            .get_object::<gtk::Button>("invite_button")
            .expect("Can't find invite_button in ui file.");
        let invite_entry_box = self.ui.builder
            .get_object::<gtk::Box>("invite_entry_box")
            .expect("Can't find invite_entry_box in ui file.");
        let invite_entry = self.ui.builder
            .get_object::<gtk::TextView>("invite_entry")
            .expect("Can't find invite_entry in ui file.");
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");

        if let Some(buffer) = invite_entry.get_buffer() {
            let placeholder_tag = gtk::TextTag::new(Some("placeholder"));

            placeholder_tag.set_property_foreground_rgba(Some(&gdk::RGBA {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 0.5,
            }));

            if let Some(tag_table) = buffer.get_tag_table() {
                tag_table.add(&placeholder_tag);
            }
        }

        // this is used to cancel the timeout and not search for every key input. We'll wait 500ms
        // without key release event to launch the search
        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        invite_entry.connect_key_release_event(clone!(op => move |entry, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            let sid = gtk::timeout_add(500, clone!(op, entry, source_id => move || {
                if let Some(buffer) = entry.get_buffer() {
                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    let text = buffer.get_text(&start, &end, false);

                    op.lock().unwrap().search_invite_user(text);
                }

                *(source_id.lock().unwrap()) = None;
                gtk::Continue(false)
            }));

            *(source_id.lock().unwrap()) = Some(sid);
            glib::signal::Inhibit(false)
        }));

        invite_entry.connect_focus_in_event(clone!(op, invite_entry_box => move |_, _| {
            if let Some(style) = invite_entry_box.get_style_context() {
                style.add_class("message-input-focused");
            }

            op.lock().unwrap().remove_invite_user_dialog_placeholder();

            Inhibit(false)
        }));

        invite_entry.connect_focus_out_event(clone!(op, invite_entry_box => move |_, _| {
            if let Some(style) = invite_entry_box.get_style_context() {
                style.remove_class("message-input-focused");
            }

            op.lock().unwrap().set_invite_user_dialog_placeholder();

            Inhibit(false)
        }));

        if let Some(buffer) = invite_entry.get_buffer() {
            buffer.connect_delete_range(clone!( op => move |_, _, _| {
                gtk::idle_add(clone!(op => move || {
                    op.lock().unwrap().detect_removed_invite();
                    Continue(false)
                }));
            }));
        }

        dialog.connect_delete_event(clone!(op => move |_, _| {
            op.lock().unwrap().close_invite_dialog();
            glib::signal::Inhibit(true)
        }));
        cancel.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_invite_dialog();
        }));
        invite.set_sensitive(false);
        invite.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().invite();
        }));
    }
}
