use crate::JsonValue;
use glib;
use regex;
use reqwest;
use serde_json;
use std::{ffi::OsString, io, time::SystemTimeError};
use url;

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
    fn from(err: reqwest::Error) -> Self {
        Error::ReqwestError(err)
    }
}

derror!(url::ParseError, Error::BackendError);
derror!(io::Error, Error::BackendError);
derror!(regex::Error, Error::BackendError);
derror!(glib::Error, Error::BackendError);
derror!(SystemTimeError, Error::BackendError);
derror!(OsString, Error::CacheError);
derror!(serde_json::Error, Error::CacheError);
