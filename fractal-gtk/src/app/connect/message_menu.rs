extern crate gtk;

use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_message_menu(&self) {
        let view_source_button: gtk::ModelButton = self.ui.builder
            .get_object("view_source_button")
            .expect("Can't find view_source_button in ui file.");

        let message_menu = self.op.lock().unwrap().message_menu.clone();
        view_source_button.connect_clicked(move |_| {
            if let Some(message_menu) = message_menu.read().unwrap().clone() {
                message_menu.display_source_dialog();
            }
        });
    }
}
