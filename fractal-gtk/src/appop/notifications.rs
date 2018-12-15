use appop::AppOp;

impl AppOp {
    pub fn clear_room_notifications(&mut self, _r: String) {}

    pub fn set_room_notifications(&mut self, _roomid: String, _n: i32, _h: i32) {}
}
