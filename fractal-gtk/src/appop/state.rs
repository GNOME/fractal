extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::room::RoomPanel;


#[derive(Debug, Clone)]
pub enum AppState {
    Login,
    Chat,
    Directory,
    Loading,
    AccountSettings,
    MediaViewer,
}


impl AppOp {
    pub fn set_state(&mut self, state: AppState) {
        self.state = state;

        let widget_name = match self.state {
            AppState::Login => {
                self.clean_login();
                "login"
            },
            AppState::Chat => "chat",
            AppState::Directory => "directory",
            AppState::Loading => "loading",
            AppState::AccountSettings => "account-settings",
            AppState::MediaViewer => "media-viewer",
        };

        self.ui.builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.")
            .set_visible_child_name(widget_name);

        //setting headerbar
        let bar_name = match self.state {
            AppState::Login => "login",
            AppState::Directory => "back",
            AppState::Loading => "login",
            AppState::AccountSettings => "account-settings",
            AppState::MediaViewer => "media-viewer",
            _ => "normal",
        };

        self.ui.builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.")
            .set_visible_child_name(bar_name);

        //set focus for views
        let widget_focus = match self.state {
            AppState::Login => "login_username",
            AppState::Directory => "directory_search_entry",
            _ => "",
        };

        if widget_focus != "" {
            self.ui.builder
                .get_object::<gtk::Widget>(widget_focus)
                .expect("Can't find widget to set focus in ui file.")
                .grab_focus();
        }

        if let AppState::Directory = self.state {
            self.search_rooms(false);
        }
    }

    pub fn escape(&mut self) {
        if let AppState::Chat = self.state {
            self.room_panel(RoomPanel::NoRoom);
            self.active_room = None;
            self.clear_tmp_msgs();
        }
    }
}
