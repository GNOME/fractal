extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Room;

impl AppOp {
    pub fn display_media_viewer(&mut self, url: String, room_id: String) {
        self.set_state(AppState::MediaViewer);

        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let image = image::Image::new(&self.backend,
                                      &url,
                                      None,
                                      image::Thumb(false),
                                      image::Circle(false),
                                      image::Fixed(true));

        media_viewport.add(&image.widget);
        media_viewport.show_all();
    }
}
