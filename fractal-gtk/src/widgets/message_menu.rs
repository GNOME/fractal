extern crate gdk;
extern crate gtk;
extern crate sourceview;

use self::gtk::prelude::*;

use types::Message;

use uibuilder::UI;

#[derive(Clone)]
pub struct MessageMenu {
    ui: UI,
    msg: Message,
}

impl MessageMenu {
    pub fn new_message_menu(ui: UI, msg: Message) -> MessageMenu {
        MessageMenu {
            ui,
            msg,
        }
    }

    pub fn show_menu_popover(&self, w: gtk::Widget) {
        let menu_popover: gtk::Popover = self.ui.builder
            .get_object("message_menu_popover")
            .expect("Can't find message_menu_popover in ui file.");
        menu_popover.set_relative_to(&w);

        menu_popover.popup();
    }

    pub fn insert_quote(&self) {
        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Can't find msg_entry in ui file.");

        if let Some(buffer) = msg_entry.get_buffer() {
            let quote = self.msg.body.lines().map(|l| "> ".to_owned() + l + "\n")
                                .collect::<Vec<String>>().join("\n") + "\n";

            let mut start = buffer.get_start_iter();
            buffer.insert(&mut start, &quote);

            msg_entry.grab_focus();
        }
    }

    pub fn copy_text(&self) {
        let atom = gdk::Atom::intern("CLIPBOARD");
        let clipboard = gtk::Clipboard::get(&atom);

        clipboard.set_text(&self.msg.body);
    }

    pub fn display_source_dialog(&self) {
        let dialog: gtk::MessageDialog = self.ui.builder
            .get_object("source_dialog")
            .expect("Can't find source_dialog in ui file.");

        dialog.set_property_secondary_text(Some(
            self.msg.source.clone().unwrap_or("This message has no source.".to_string()).as_str()
        ));

        dialog.connect_response(move |d, res| {
            if gtk::ResponseType::from(res) == gtk::ResponseType::Accept {
                let atom = gdk::Atom::intern("CLIPBOARD");
                let clipboard = gtk::Clipboard::get(&atom);

                if let Some(src) = d.get_property_secondary_text() {
                    clipboard.set_text(&src);
                }
            } else {
                d.hide();
            }
        });

        dialog.show();
    }
}
