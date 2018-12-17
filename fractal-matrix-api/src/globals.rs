pub static TIMEOUT: u64 = 80;
pub static PAGE_LIMIT: i32 = 40;
pub static ROOM_DIRECTORY_LIMIT: i32 = 20;
pub static THUMBNAIL_SIZE: i32 = 128;
pub static MATRIX_RE: &str = r"mxc://(?P<server>[^/]+)/(?P<media>.+)";
pub static EMAIL_RE: &str =
    r"^([0-9a-zA-Z]([-\.\w]*[0-9a-zA-Z])+@([0-9a-zA-Z][-\w]*[0-9a-zA-Z]\.)+[a-zA-Z]{2,9})$";
