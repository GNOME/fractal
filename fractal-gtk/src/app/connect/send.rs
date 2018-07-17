extern crate gdk;
extern crate gtk;
extern crate sourceview;

use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_send(&self) {
        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        let autocomplete_popover = self.ui.builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");

        let mut op = self.op.clone();
        msg_entry.connect_key_press_event(move |entry, key| {
            match key.get_keyval() {
                gdk::enums::key::Return | gdk::enums::key::KP_Enter
                if !key.get_state().contains(gdk::ModifierType::CONTROL_MASK) &&
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
    }
}
