use crate::app::App;
use crate::i18n::i18n;
use lazy_static::lazy_static;
use log::error;
use regex::Regex;

use crate::actions::{activate_action, AppState};

use crate::error::BKError;
use crate::error::Error;

pub fn dispatch_error(error: BKError) {
    match error {
        BKError::AccountDestructionError(err) => {
            let error = i18n("Couldn’t delete the account");
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::ChangePasswordError(err) => {
            let error = i18n("Couldn’t change the password");
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(show_password_error_dialog, (error));
        }
        BKError::GetThreePIDError(_) => {
            let error = i18n("Sorry, account settings can’t be loaded.");
            APPOP!(show_load_settings_error_dialog, (error));
            let ctx = glib::MainContext::default();
            ctx.invoke(move || {
                activate_action("app", "back");
            })
        }
        BKError::GetTokenEmailError(Error::TokenUsed) => {
            let error = i18n("Email is already in use");
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::GetTokenEmailError(Error::Denied) => {
            let error = i18n("Please enter a valid email address.");
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::GetTokenEmailError(err) => {
            let error = i18n("Couldn’t add the email address.");
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::GetTokenPhoneError(Error::TokenUsed) => {
            let error = i18n("Phone number is already in use");
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::GetTokenPhoneError(Error::Denied) => {
            let error = i18n(
                "Please enter your phone number in the format: \n + your country code and your phone number.",
            );
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::GetTokenPhoneError(err) => {
            let error = i18n("Couldn’t add the phone number.");
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(show_error_dialog_in_settings, (error));
        }
        BKError::NewRoomError(err, internal_id) => {
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );

            let error = i18n("Can’t create the room, try again");
            let state = AppState::NoRoom;
            APPOP!(remove_room, (internal_id));
            APPOP!(show_error, (error));
            APPOP!(set_state, (state));
        }
        BKError::JoinRoomError(err) => {
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            let error = i18n("Can’t join the room, try again.").to_string();
            let state = AppState::NoRoom;
            APPOP!(show_error, (error));
            APPOP!(set_state, (state));
        }
        BKError::ChangeLanguageError(err) => {
            let err_str = format!("{:?}", err);
            error!(
                "Error forming url to set room language: {}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
        }
        BKError::LoginError(_) => {
            let error = i18n("Can’t login, try again");
            let st = AppState::Login;
            APPOP!(show_error, (error));
            APPOP!(logout);
            APPOP!(set_state, (st));
        }
        BKError::AttachedFileError(err) => {
            let err_str = format!("{:?}", err);
            error!(
                "attaching {}: retrying send",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(retry_send);
        }
        BKError::SentMsgError(Error::SendMsgError(txid)) => {
            error!("sending {}: retrying send", txid);
            APPOP!(retry_send);
        }
        BKError::SentMsgError(_) => {
            let error = i18n("Error sending message");
            APPOP!(show_error, (error));
        }
        BKError::SentMsgRedactionError(_) => {
            let error = i18n("Error deleting message");
            APPOP!(show_error, (error));
        }
        BKError::DirectoryProtocolsError(_) | BKError::DirectorySearchError(_) => {
            let error = i18n("Error searching for rooms");
            APPOP!(reset_directory_state);
            APPOP!(show_error, (error));
        }
        BKError::SyncError(err, number_tries) => {
            let err_str = format!("{:?}", err);
            error!(
                "SYNC Error: {}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            let new_number_tries = number_tries + 1;
            APPOP!(sync_error, (new_number_tries));
        }
        err => {
            let err_str = format!("{:?}", err);
            error!(
                "Query error: {}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
        }
    }
}

/// This function removes the value of the `access_token` query from a URL used for accessing the Matrix API.
/// The primary use case is the removing of sensitive information for logging.
/// Specifically, the URL is expected to be contained within quotes and the token is replaced with `<redacted>`.
/// Returns `Some` on removal, otherwise `None`.
fn remove_matrix_access_token_if_present(message: &str) -> Option<String> {
    lazy_static! {
    static ref RE: Regex =
        Regex::new(r#""((http)|(https))://([^"]+)/_matrix/([^"]+)\?access_token=(?P<token>[^&"]+)([^"]*)""#,)
        .expect("Malformed regular expression.");
    }
    // If the supplied string doesn't contain a match for the regex, we return `None`.
    let cap = RE.captures(message)?;
    let captured_token = cap
        .name("token")
        .expect("'token' capture group not present.")
        .as_str();
    Some(message.replace(captured_token, "<redacted>"))
}
