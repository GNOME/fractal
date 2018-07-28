extern crate cairo;
extern crate url;
extern crate regex;
extern crate reqwest;
extern crate glib;
extern crate serde_json;

use std::io;
use std::time::SystemTimeError;
use std::ffi::OsString;

use self::serde_json::Value as JsonValue;

#[derive(Debug)]
pub enum Error {
    BackendError,
    CacheError,
    ReqwestError(reqwest::Error),
    MatrixError(JsonValue),
    SendMsgError(String),
    SendMsgRedactionError(String),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::ReqwestError(err)
    }
}

derror!(url::ParseError, Error::BackendError);
derror!(io::Error, Error::BackendError);
derror!(regex::Error, Error::BackendError);
derror!(cairo::Status, Error::BackendError);
derror!(cairo::IoError, Error::BackendError);
derror!(glib::Error, Error::BackendError);
derror!(SystemTimeError, Error::BackendError);

derror!(OsString, Error::CacheError);
derror!(serde_json::Error, Error::CacheError);
