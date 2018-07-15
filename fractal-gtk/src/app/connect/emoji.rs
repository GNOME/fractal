extern crate gtk;
extern crate sourceview;

use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_emoji(&self) {
        let emoji_button: gtk::Button = self.ui.builder
            .get_object("emoji_button")
            .expect("Couldn't find emoji_button in ui file.");

        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        emoji_button.connect_clicked(move |_| {
            msg_entry.grab_focus();
            // This is a workaroung as the `emit_insert_emoji` doesn't exist
            msg_entry.emit("insert-emoji", &[]);
        });
    }
}
