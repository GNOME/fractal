use crate::appop::AppOp;

impl AppOp {
    pub fn clear_room_notifications(&mut self, r: String) {
        self.set_room_notifications(r.clone(), 0, 0);
        self.roomlist.set_bold(r, false);
    }

    pub fn set_room_notifications(&mut self, roomid: String, n: i32, h: i32) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.notifications = n;
            r.highlight = h;
            self.roomlist
                .set_room_notifications(roomid, r.notifications, r.highlight);
        }
    }
}
