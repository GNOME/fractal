extern crate gtk;

use self::gtk::prelude::*;

use appop::AppState;

use app::App;

impl App {
    pub fn connect_media_viewer_headerbar(&self) {
        let ui = self.ui.clone();
        let op = self.op.clone();
        let btn = ui.builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Cant find media_viewer_back_button in ui file.");
        btn.connect_clicked(move |_| {
            let media_viewport = ui.builder
                .get_object::<gtk::Viewport>("media_viewport")
                .expect("Cant find media_viewport in ui file.");
            if let Some(child) = media_viewport.get_child() {
                media_viewport.remove(&child);
            }

            op.lock().unwrap().set_state(AppState::Chat);
        });
    }

    pub fn connect_media_viewer_box(&self) {

    }
}
