extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use app::InternalCommand;

/* needed to request members list */
/*
   use backend::BKCommand;
   use fractal_api::types::Member;
   use std::sync::mpsc::channel;
   use std::sync::mpsc::TryRecvError;
   use std::sync::mpsc::{Receiver, Sender};
   */

use widgets;

impl AppOp {
    pub fn show_room_settings(&mut self) {
        self.create_room_settings();
    }

    pub fn create_room_settings(&mut self) -> Option<()> {
        let stack = self.ui
            .builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.");
        let stack_header = self.ui
            .builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");

        {
            let room = self.rooms.get(&self.active_room.clone()?)?;
            let mut panel =
                widgets::RoomSettings::new(self.backend.clone(), self.uid.clone(), room.clone());
            let (body, header) = panel.create()?;

            /* remove old panel */
            if let Some(widget) = stack.get_child_by_name("room-settings") {
                stack.remove(&widget);
            }
            if let Some(widget) = stack_header.get_child_by_name("room-settings") {
                stack_header.remove(&widget);
            }

            stack.add_named(&body, "room-settings");
            stack_header.add_named(&header, "room-settings");

            /* Headerbar */
            let internal = self.internal.clone();
            panel.get_back_button()?.connect_clicked(move |_| {
                internal
                    .send(InternalCommand::SetView(AppState::Chat))
                    .unwrap()
            });

            self.room_settings = Some(panel);
        }

        self.set_state(AppState::RoomSettings);

        None
    }

    pub fn close_room_settings(&mut self) {
        let stack = self.ui
            .builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.");
        let stack_header = self.ui
            .builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");

        /* remove old panel */
        if let Some(widget) = stack.get_child_by_name("room-settings") {
            stack.remove(&widget);
        }
        if let Some(widget) = stack_header.get_child_by_name("room-settings") {
            stack_header.remove(&widget);
        }

        self.room_settings = None;
    }

    pub fn show_new_room_avatar(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_avatar();
        None
    }

    pub fn show_new_room_name(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_name();
        None
    }

    pub fn show_new_room_topic(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_topic();
        None
    }

    /*
       pub fn update_members_list(&self, uid: String) -> Option<()> {
       self.room_settings.clone()?.update_members_list(uid);
       None
       }
       */

    /*
       pub fn request_members_list(&self) -> Option<()> {
       let room = self.rooms.get(&self.active_room.clone()?)?;
       let members: Vec<Member> = room.members.values().cloned().collect();
       for member in members.iter() {
       account_settings_get_member_info(
       self.backend.clone(),
       member.uid.clone());
       }

       None

*/
}

/* this funtion should be moved to the backend */
/*
   pub fn account_settings_get_member_info(
   backend: Sender<BKCommand>,
   sender: String,
   ) {
   let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
   backend
   .send(BKCommand::GetUserInfoAsync(sender.clone(), Some(tx)))
   .unwrap();
   gtk::timeout_add(100, move || match rx.try_recv() {
   Err(TryRecvError::Empty) => gtk::Continue(true),
   Err(TryRecvError::Disconnected) => gtk::Continue(false),
   Ok((_name, _avatar)) => {
/* update UI */
//view.update_members_list(index);
//APPOP!(update_members_list, (sender));
gtk::Continue(false)
    }
});
}
*/
