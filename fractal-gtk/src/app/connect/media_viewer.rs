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
            op.lock().unwrap().hide_media_viewer();
        });
    }

    pub fn connect_media_viewer_box(&self) {
        let op = self.op.clone();
        let previous_media_button = self.ui.builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");
        previous_media_button.connect_clicked(move |_| {
            op.lock().unwrap().previous_media();
        });

        let op = self.op.clone();
        let next_media_button = self.ui.builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");
        next_media_button.connect_clicked(move |_| {
            op.lock().unwrap().next_media();
        });
    }
}
