use glib;
use std::io;
use std::time::SystemTimeError;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct StandardErrorResponse {
    pub errcode: String,
    pub error: String,
}

type MatrixErrorCode = String;

#[macro_export]
macro_rules! derror {
    ($from: path, $to: path) => {
        impl From<$from> for Error {
            fn from(_: $from) -> Error {
                $to
            }
        }
    };
}

#[derive(Debug)]
pub enum Error {
    BackendError,
    CacheError,
    ReqwestError(reqwest::Error),
    NetworkError(reqwest::StatusCode),
    MatrixError(MatrixErrorCode, String),
    SendMsgError(String),
    SendMsgRedactionError(String),
    TokenUsed,
    Denied,
    NotLoggedIn,
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::ReqwestError(err)
    }
}

impl From<StandardErrorResponse> for Error {
    fn from(resp: StandardErrorResponse) -> Error {
        Error::MatrixError(resp.errcode, resp.error)
    }
}

derror!(url::ParseError, Error::BackendError);
derror!(io::Error, Error::BackendError);
derror!(glib::error::Error, Error::BackendError);
derror!(regex::Error, Error::BackendError);
derror!(ruma_identifiers::Error, Error::BackendError);
derror!(SystemTimeError, Error::BackendError);

derror!(serde_json::Error, Error::CacheError);
