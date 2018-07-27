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
}
