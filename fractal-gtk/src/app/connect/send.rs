extern crate sourceview;

use gdk;
use gtk;
use gtk::prelude::*;
use self::sourceview::BufferExt;

use app::App;

const MAX_INPUT_HEIGHT: i32 = 100;

impl App {
    pub fn connect_send(&self) {
        self.ui.sventry.container.set_redraw_on_allocate(true);
        let msg_entry = self.ui.sventry.view.clone();
        let buffer = &self.ui.sventry.buffer;
        buffer.set_highlight_matching_brackets(false);

        let msg_entry_box = self.ui.sventry.entry_box.clone();
        msg_entry_box.set_redraw_on_allocate(true);

        if let Some(adjustment) = self.ui.sventry.scroll.get_vadjustment() {
            adjustment.connect_value_changed(clone!(msg_entry => move |adj| {
                if msg_entry.get_allocated_height() < MAX_INPUT_HEIGHT {
                    adj.set_value(0.0);
                }
            }));
        }


        let autocomplete_popover = self.ui.builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");

        let mut op = self.op.clone();
        msg_entry.connect_key_press_event(move |entry, key| {
            match key.get_keyval() {
                gdk::enums::key::Return | gdk::enums::key::KP_Enter
                if !key.get_state().contains(gdk::ModifierType::SHIFT_MASK) &&
                   !autocomplete_popover.is_visible() => {
                    if let Some(buffer) = entry.get_buffer() {
                        let start = buffer.get_start_iter();
                        let end = buffer.get_end_iter();

                        if let Some(text) = buffer.get_text(&start, &end, false) {
                            let mut mut_text = text;
                            op.lock().unwrap().send_message(mut_text);
                        }

                        buffer.set_text("");
                    }

                    Inhibit(true)
                },
                _ => Inhibit(false)
            }
        });

        op = self.op.clone();
        msg_entry.connect_paste_clipboard(move |_| {
            op.lock().unwrap().paste();
        });

        msg_entry.connect_focus_in_event(clone!(msg_entry_box => move |_, _| {
            if let Some(style) = msg_entry_box.get_style_context() {
                style.add_class("message-input-focused");
            }

            Inhibit(false)
        }));

        msg_entry.connect_focus_out_event(clone!(msg_entry_box => move |_, _| {
            if let Some(style) = msg_entry_box.get_style_context() {
                style.remove_class("message-input-focused");
            }

            Inhibit(false)
        }));
    }
}
