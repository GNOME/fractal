extern crate gtk;
extern crate gdk;

use self::gtk::prelude::*;
use self::gdk::WindowExt;

use appop::AppOp;
use appop::room::RoomPanel;


#[derive(Debug, Clone)]
pub enum AppState {
    Login,
    Chat,
    Directory,
    Loading,
    AccountSettings,
    RoomSettings,
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
            AppState::RoomSettings => "room-settings",
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
            AppState::RoomSettings => "room-settings",
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

    pub fn escape(&mut self, w: &gtk::ApplicationWindow) -> bool {
        if self.inhibit_escape {
            return true;
        }

        // leave full screen only if we're currently in fullscreen
        if let Some(win) = w.get_window() {
            if win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                self.leave_full_screen();
                return true;
            }
        }

        match self.state {
            AppState::Chat => {
                self.room_panel(RoomPanel::NoRoom);
                self.active_room = None;
                self.clear_tmp_msgs();
                true
            },
            AppState::MediaViewer => {
                self.hide_media_viewer();
                true
            },
            _ => { false }
        }
    }

    pub fn left(&mut self) -> bool {
        match self.state {
            AppState::MediaViewer => {
                if self.media_viewer.is_none() {
                    return false;
                }

                let mv = self.media_viewer.clone().unwrap();
                let loading_more_media = *mv.loading_more_media.read().unwrap();
                let no_more_media = *mv.no_more_media.read().unwrap();
                if loading_more_media || no_more_media {
                    return false;
                }

                self.previous_media();
                true
            },
            _ => { false }
        }
    }

    pub fn right(&mut self) -> bool {
        match self.state {
            AppState::MediaViewer => {
                if self.media_viewer.is_none() {
                    return false;
                }

                let mv = self.media_viewer.clone().unwrap();
                let loading_more_media = *mv.loading_more_media.read().unwrap();
                if loading_more_media {
                    return false;
                }

                self.next_media();
                true
            },
            _ => { false }
        }
    }
}
