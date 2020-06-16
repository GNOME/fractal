use gtk::prelude::*;
use libhandy::LeafletExt;

use crate::actions::AppState;
use crate::appop::AppOp;

impl AppOp {
    pub fn set_state(&mut self, state: AppState) {
        self.state = state;
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("room_view_stack")
            .expect("Can't find room_view_stack in ui file.");
        let headerbar = self
            .ui
            .builder
            .get_object::<gtk::HeaderBar>("room_header_bar")
            .expect("Can't find room_header_bar in ui file.");

        let widget_name = match self.state {
            AppState::Login => "login",
            AppState::NoRoom => {
                self.set_state_no_room(&headerbar);
                self.leaflet.set_visible_child_name("sidebar");
                stack.set_visible_child_name("noroom");
                "chat"
            }
            AppState::Room => {
                self.set_state_room(&headerbar);
                self.leaflet.set_visible_child_name("content");
                stack.set_visible_child_name("room_view");
                "chat"
            }
            AppState::Directory => "directory",
            AppState::Loading => "loading",
            AppState::AccountSettings => "account-settings",
            AppState::RoomSettings => "room-settings",
            AppState::MediaViewer => "media-viewer",
        };

        self.ui
            .builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.")
            .set_visible_child_name(widget_name);

        //setting headerbar
        let bar_name = match self.state {
            AppState::Login => "login",
            AppState::Directory => "back",
            AppState::Loading => "loading",
            AppState::AccountSettings => "account-settings",
            AppState::RoomSettings => "room-settings",
            AppState::MediaViewer => "media-viewer",
            _ => "normal",
        };

        self.ui
            .builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.")
            .set_visible_child_name(bar_name);

        //set focus for views
        let widget_focus = match self.state {
            AppState::Directory => "directory_search_entry",
            _ => "",
        };

        if widget_focus != "" {
            self.ui
                .builder
                .get_object::<gtk::Widget>(widget_focus)
                .expect("Can't find widget to set focus in ui file.")
                .grab_focus();
        }

        if let AppState::Directory = self.state {
            self.search_rooms(false);
        }
    }

    fn set_state_room(&self, headerbar: &gtk::HeaderBar) {
        for ch in headerbar.get_children().iter() {
            ch.show();
        }

        self.ui.sventry.view.grab_focus();

        let msg = self
            .active_room
            .as_ref()
            .and_then(|active_room_id| self.unsent_messages.get(active_room_id))
            .cloned()
            .unwrap_or_default();
        if let Some(buffer) = self.ui.sventry.view.get_buffer() {
            buffer.set_text(&msg.0);

            let iter = buffer.get_iter_at_offset(msg.1);
            buffer.place_cursor(&iter);
        }
    }

    // WORKAROUND this is needed because NoRoom isn't a real app state
    fn set_state_no_room(&mut self, headerbar: &gtk::HeaderBar) {
        for ch in headerbar.get_children().iter() {
            ch.hide();

            // Select new active room in the sidebar
            self.roomlist.unselect();
        }
        self.active_room = None;
        self.clear_tmp_msgs();
    }
}
