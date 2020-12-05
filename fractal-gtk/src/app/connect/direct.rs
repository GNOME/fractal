use glib::clone;
use gtk::prelude::*;

use glib::source::Continue;
use std::sync::{Arc, Mutex};

use crate::app::{self, App};

impl App {
    pub fn connect_direct_chat(&self) {
        let cancel = self
            .ui
            .builder
            .get_object::<gtk::Button>("cancel_direct_chat")
            .expect("Can't find cancel_direct_chat in ui file.");
        let invite = self
            .ui
            .builder
            .get_object::<gtk::Button>("direct_chat_button")
            .expect("Can't find direct_chat_button in ui file.");
        let to_chat_entry_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("to_chat_entry_box")
            .expect("Can't find to_chat_entry_box in ui file.");
        let to_chat_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");

        if let Some(buffer) = to_chat_entry.get_buffer() {
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
        to_chat_entry.connect_key_release_event(move |entry, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            let sid = glib::timeout_add_local(
                500,
                clone!(
                @strong entry,
                @strong source_id
                => move || {
                    if let Some(buffer) = entry.get_buffer() {
                        let start = buffer.get_start_iter();
                        let end = buffer.get_end_iter();

                        if let Some(text) =
                            buffer.get_text(&start, &end, false).map(|gstr| gstr.to_string())
                        {
                            let _ = app::get_app_tx().send(Box::new(|op| op.search_invite_user(text)));
                        }
                    }

                    *(source_id.lock().unwrap()) = None;
                    Continue(false)
                }),
            );

            *(source_id.lock().unwrap()) = Some(sid);
            glib::signal::Inhibit(false)
        });

        to_chat_entry.connect_focus_in_event(clone!(@strong to_chat_entry_box => move |_, _| {
            to_chat_entry_box.get_style_context().add_class("message-input-focused");

            let _ = app::get_app_tx().send(Box::new(|op| op.remove_invite_user_dialog_placeholder()));

            Inhibit(false)
        }));

        to_chat_entry.connect_focus_out_event(clone!(@strong to_chat_entry_box => move |_, _| {
            to_chat_entry_box.get_style_context().remove_class("message-input-focused");

            let _ = app::get_app_tx().send(Box::new(|op| op.set_invite_user_dialog_placeholder()));

            Inhibit(false)
        }));

        if let Some(buffer) = to_chat_entry.get_buffer() {
            buffer.connect_delete_range(move |_, _, _| {
                glib::idle_add_local(move || {
                    let _ = app::get_app_tx().send(Box::new(|op| op.detect_removed_invite()));
                    Continue(false)
                });
            });
        }

        dialog.connect_delete_event(move |_, _| {
            let _ = app::get_app_tx().send(Box::new(|op| op.close_direct_chat_dialog()));
            glib::signal::Inhibit(true)
        });
        cancel.connect_clicked(move |_| {
            let _ = app::get_app_tx().send(Box::new(|op| op.close_direct_chat_dialog()));
        });
        invite.set_sensitive(false);
        invite.connect_clicked(move |_| {
            let _ = app::get_app_tx().send(Box::new(|op| op.start_chat()));
        });
    }
}
