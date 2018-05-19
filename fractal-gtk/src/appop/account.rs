extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;

use backend::BKCommand;
use widgets;
use widgets::AvatarExt;

impl AppOp {
    pub fn show_account_settings_dialog(&self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("account_settings_dialog")
            .expect("Can't find account_settings_dialog in ui file.");
        let avatar = self.ui.builder
            .get_object::<gtk::Container>("account_settings_avatar")
            .expect("Can't find account_settings_avatar in ui file.");
        let name = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let uid = self.ui.builder
            .get_object::<gtk::Label>("account_settings_uid")
            .expect("Can't find account_settings_uid in ui file.");
        let homeserver = self.ui.builder
            .get_object::<gtk::Label>("account_settings_homeserver")
            .expect("Can't find account_settings_homeserver in ui file.");

        /* remove all old avatar from the popover */
        for w in avatar.get_children().iter() {
            avatar.remove(w);
        }

        uid.set_text(&self.uid.clone().unwrap_or_default());
        homeserver.set_text(&self.server_url);
        name.set_text(&self.username.clone().unwrap_or_default());
        name.grab_focus_without_selecting();
        name.set_position(-1);

        let w = widgets::Avatar::circle_avatar(self.avatar.clone().unwrap_or_default(), Some(100));
        avatar.add(&w);
        avatar.show();

        dialog.present();
    }

    pub fn apply_account_settings(&self) {
        let name = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");

        let old_username = self.username.clone().unwrap_or_default();
        let username = name.get_text().unwrap_or_default();

        if old_username !=  username {
            self.backend.send(BKCommand::SetUserName(username)).unwrap();
        }
    }

    pub fn close_account_settings_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("account_settings_dialog")
            .expect("Can't find account_settings_dialog in ui file.");
        /*
        let avatar = self.ui.builder
            .get_object::<gtk::Container>("account_settings_avatar")
            .expect("Can't find account_settings_avatar in ui file.");
        let name = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        */
        let advanced = self.ui.builder                                                                          
            .get_object::<gtk::Revealer>("account_settings_advanced")                                 
            .expect("Can't find account_settings_advanced in ui file."); 
        let delete = self.ui.builder                                                                          
            .get_object::<gtk::Revealer>("account_settings_delete")                                 
            .expect("Can't find account_settings_delete in ui file."); 

        let advanced_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_advanced_toggle")                              
            .expect("Can't find account_settings_advanced_toggle in ui file.");                           
        let delete_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_delete_toggle")                                
            .expect("Can't find account_settings_delete_toggle in ui file.");

        advanced_toggle.get_style_context().unwrap().remove_class("advanced_revealer_divider");
        delete_toggle.get_style_context().unwrap().remove_class("advanced_revealer_divider");
        advanced.set_reveal_child(false);
        delete.set_reveal_child(false);
        dialog.hide();
        dialog.resize(700, 200);
    }
}
