use crate::appop::AppOp;
use fractal_api::identifiers::RoomId;

impl AppOp {
    pub fn clear_room_notifications(&mut self, room_id: RoomId) {
        self.set_room_notifications(room_id.clone(), 0, 0);
        self.roomlist.set_bold(room_id, false);
        self.update_title();
    }

    pub fn set_room_notifications(&mut self, room_id: RoomId, n: i32, h: i32) {
        if let Some(r) = self.rooms.get_mut(&room_id) {
            r.notifications = n;
            r.highlight = h;
            self.roomlist
                .set_room_notifications(room_id, r.notifications, r.highlight);
        }
        self.update_title();
    }
}
