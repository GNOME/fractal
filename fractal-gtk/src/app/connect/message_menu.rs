extern crate gtk;

use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_message_menu(&self) {
        let reply_button: gtk::ModelButton = self.ui.builder
            .get_object("reply_button")
            .expect("Can't find reply_button in ui file.");

        let copy_text_button: gtk::ModelButton = self.ui.builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button in ui file.");

        let view_source_button: gtk::ModelButton = self.ui.builder
            .get_object("view_source_button")
            .expect("Can't find view_source_button in ui file.");

        let message_menu = self.op.lock().unwrap().message_menu.clone();
        reply_button.connect_clicked(move |_| {
            if let Some(message_menu) = message_menu.read().unwrap().clone() {
                message_menu.insert_quote();
            }
        });

        let message_menu = self.op.lock().unwrap().message_menu.clone();
        copy_text_button.connect_clicked(move |_| {
            if let Some(message_menu) = message_menu.read().unwrap().clone() {
                message_menu.copy_text();
            }
        });

        let message_menu = self.op.lock().unwrap().message_menu.clone();
        view_source_button.connect_clicked(move |_| {
            if let Some(message_menu) = message_menu.read().unwrap().clone() {
                message_menu.display_source_dialog();
            }
        });
    }
}
