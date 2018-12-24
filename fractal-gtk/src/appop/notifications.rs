use crate::appop::AppOp;

impl AppOp {
    pub fn clear_room_notifications(&mut self, room_id: String) {
        self.set_room_notifications(room_id, 0, 0);
    }

    pub fn set_room_notifications(&mut self, room_id: String, n: i32, h: i32) {
        if let Some(r) = self.rooms.get_mut(&room_id) {
            r.notifications = n;
            r.highlight = h;
            self.sidebar_store.update_room(&r);
        }
    }
}
