extern crate gtk;

use self::gtk::prelude::*;

use appop::AppState;

use app::App;

impl App {
    pub fn connect_media_viewer_headerbar(&self) {
        let op = self.op.clone();
        let btn = self.ui.builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Cant find media_viewer_back_button in ui file.");
        btn.connect_clicked(move |_| {
            op.lock().unwrap().set_state(AppState::Chat);
        });
    }

    pub fn connect_media_viewer_box(&self) {

    }
}
