use gtk;
use gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use widgets;

use types::Message;

impl AppOp {
    /* FIXME: take msg by reference and maybe create an action for this */
    pub fn create_media_viewer(&mut self, msg: Message) -> Option<()> {
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.");
        let stack_header = self
            .ui
            .builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");

        let main_window = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");

        {
            let room_id = self.active_room.as_ref()?;
            let room = self.rooms.get(room_id)?;
            let mut panel =
                widgets::MediaViewer::new(self.backend.clone(), main_window.clone(), room, &msg);
            panel.display_media_viewer(msg);
            let (body, header) = panel.create()?;

            /* remove old panel */
            if let Some(widget) = stack.get_child_by_name("media-viewer") {
                stack.remove(&widget);
            }
            if let Some(widget) = stack_header.get_child_by_name("media-viewer") {
                stack_header.remove(&widget);
            }

            stack.add_named(&body, "media-viewer");
            stack_header.add_named(&header, "media-viewer");
        }

        self.set_state(AppState::MediaViewer);

        None
    }
}
