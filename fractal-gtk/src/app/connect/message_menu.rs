extern crate gtk;

use self::gtk::prelude::*;

use app::App;

use backend::BKCommand;

impl App {
    pub fn connect_message_menu(&self) {
        let reply_button: gtk::ModelButton = self.ui.builder
            .get_object("reply_button")
            .expect("Can't find reply_button in ui file.");

        let copy_text_button: gtk::ModelButton = self.ui.builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button in ui file.");

        let delete_message_button: gtk::ModelButton = self.ui.builder
            .get_object("delete_message_button")
            .expect("Can't find delete_message_button in ui file.");

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
        let bk = self.op.lock().unwrap().backend.clone();
        delete_message_button.connect_clicked(move |_| {
            if let Some(message_menu) = message_menu.read().unwrap().clone() {
                bk.send(BKCommand::SendMsgRedaction(message_menu.msg.clone())).unwrap();
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
