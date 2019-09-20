use lazy_static::lazy_static;
use url::Url;

pub static CACHE_SIZE: usize = 40;
pub static MSG_ICON_SIZE: i32 = 40;
pub static USERLIST_ICON_SIZE: i32 = 30;
pub static PILL_ICON_SIZE: i32 = 18;
pub static MINUTES_TO_SPLIT_MSGS: i64 = 30;
pub static DEFAULT_IDENTITYSERVER: &'static str = "https://vector.im";
pub static PLACEHOLDER_TEXT: &'static str = "Matrix username, email or phone number";
pub static RIOT_REGISTER_URL: &'static str = "https://riot.im/app/#/register";

pub static MAX_IMAGE_SIZE: (i32, i32) = (600, 400);
pub static MAX_STICKER_SIZE: (i32, i32) = (200, 130);

lazy_static! {
    pub static ref DEFAULT_HOMESERVER: Url =
        Url::parse("https://matrix.org").expect("Malformed DEFAULT_HOMESERVER value");
}
