extern crate gdk;
extern crate gtk;
extern crate sourceview;

use self::gtk::prelude::*;
use self::sourceview::BufferExt;

use app::App;

const MAX_INPUT_HEIGHT: i32 = 100;

impl App {
    pub fn connect_send(&self) {
        let room_message_box = self.ui.builder
            .get_object::<gtk::Box>("room_message_box")
            .expect("Can't find room_message_box in ui file.");
        room_message_box.set_redraw_on_allocate(true);

        let msg_entry_box = self.ui.builder
            .get_object::<gtk::Box>("msg_entry_box")
            .expect("Can't find msg_entry_box in ui file.");
        msg_entry_box.set_redraw_on_allocate(true);

        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        let input_scroll = self.ui.builder
            .get_object::<gtk::ScrolledWindow>("input_scroll")
            .expect("Can't find input_scroll in ui file.");
        if let Some(adjustment) = input_scroll.get_vadjustment() {
            adjustment.connect_value_changed(clone!(msg_entry => move |adj| {
                if msg_entry.get_allocated_height() < MAX_INPUT_HEIGHT {
                    adj.set_value(0.0);
                }
            }));
        }

        let buffer: sourceview::Buffer = self.ui.builder
            .get_object("msg_entry_buffer")
            .expect("Couldn't find msg_entry_buffer in ui file.");
        buffer.set_highlight_matching_brackets(false);

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
