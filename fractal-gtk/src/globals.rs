pub static INITIAL_MESSAGES: usize = 40;
pub static CACHE_SIZE: usize = 40;
pub static MSG_ICON_SIZE: i32 = 40;
pub static USERLIST_ICON_SIZE: i32 = 30;
pub static PILL_ICON_SIZE: i32 = 18;
pub static MINUTES_TO_SPLIT_MSGS: i64 = 30;
pub static DEFAULT_HOMESERVER: &'static str = "https://matrix.org";
pub static DEFAULT_IDENTITYSERVER: &'static str = "https://vector.im";
pub static PLACEHOLDER_TEXT: &'static str = "Matrix username, email or phone number";

pub static MAX_IMAGE_SIZE: (i32, i32) = (600, 400);
pub static MAX_STICKER_SIZE: (i32, i32) = (200, 130);

pub static LOCALEDIR: Option<&'static str> = option_env!("FRACTAL_LOCALEDIR");
pub static APP_ID: Option<&'static str> = option_env!("FRACTAL_APP_ID");
pub static NAME_SUFFIX: Option<&'static str> = option_env!("FRACTAL_NAME_SUFFIX");
pub static VERSION: Option<&'static str> = option_env!("FRACTAL_VERSION");
