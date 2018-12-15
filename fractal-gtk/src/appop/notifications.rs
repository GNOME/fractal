use appop::AppOp;

impl AppOp {
    pub fn clear_room_notifications(&mut self, r: String) {}

    pub fn set_room_notifications(&mut self, roomid: String, n: i32, h: i32) {}
}
