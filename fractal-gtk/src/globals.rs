use directories::ProjectDirs;
use fractal_api::url::Url;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;

pub static TIMEOUT: u64 = 80;
pub static PAGE_LIMIT: i32 = 40;
pub static ROOM_DIRECTORY_LIMIT: i32 = 20;
pub static DEVICE_NAME: &str = "Fractal";

pub static CACHE_SIZE: usize = 40;
pub static MSG_ICON_SIZE: i32 = 40;
pub static USERLIST_ICON_SIZE: i32 = 30;
pub static PILL_ICON_SIZE: i32 = 18;
pub static MINUTES_TO_SPLIT_MSGS: i64 = 30;
pub static PLACEHOLDER_TEXT: &str = "Matrix username, email or phone number";
pub static RIOT_REGISTER_URL: &str = "https://riot.im/app/#/register";

pub static MAX_IMAGE_SIZE: (i32, i32) = (600, 400);
pub static MAX_STICKER_SIZE: (i32, i32) = (200, 130);

lazy_static! {
    pub static ref DEFAULT_HOMESERVER: Url =
        Url::parse("https://matrix.org").expect("Malformed DEFAULT_HOMESERVER value");
    pub static ref DEFAULT_IDENTITYSERVER: Url =
        Url::parse("https://vector.im").expect("Malformed DEFAULT_IDENTITYSERVER value");
    pub static ref EMAIL_RE: Regex = Regex::new(
        r"^([0-9a-zA-Z]([-\.\w]*[0-9a-zA-Z])+@([0-9a-zA-Z][-\w]*[0-9a-zA-Z]\.)+[a-zA-Z]{2,9})$"
    )
    .unwrap();
    pub static ref CACHE_PATH: PathBuf = ProjectDirs::from("org", "GNOME", "Fractal")
        .as_ref()
        .map(ProjectDirs::cache_dir)
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("fractal"));
}
