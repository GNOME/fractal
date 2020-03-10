use lazy_static::lazy_static;
use regex::Regex;

pub static TIMEOUT: u64 = 80;
pub static PAGE_LIMIT: i32 = 40;
pub static ROOM_DIRECTORY_LIMIT: i32 = 20;
pub static DEVICE_NAME: &str = "Fractal";

lazy_static! {
    pub static ref EMAIL_RE: Regex = Regex::new(
        r"^([0-9a-zA-Z]([-\.\w]*[0-9a-zA-Z])+@([0-9a-zA-Z][-\w]*[0-9a-zA-Z]\.)+[a-zA-Z]{2,9})$"
    )
    .unwrap();
}
