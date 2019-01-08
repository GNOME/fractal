use lazy_static::lazy_static;
use regex::Regex;

pub static TIMEOUT: u64 = 80;
pub static PAGE_LIMIT: i32 = 40;
pub static ROOM_DIRECTORY_LIMIT: i32 = 20;
pub static THUMBNAIL_SIZE: i32 = 128;

lazy_static! {
    pub static ref MATRIX_RE: Regex = Regex::new(r"mxc://(?P<server>[^/]+)/(?P<media>.+)").unwrap();
    pub static ref EMAIL_RE: Regex = Regex::new(
        r"^([0-9a-zA-Z]([-\.\w]*[0-9a-zA-Z])+@([0-9a-zA-Z][-\w]*[0-9a-zA-Z]\.)+[a-zA-Z]{2,9})$"
    )
    .unwrap();
}
