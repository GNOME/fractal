use crate::app::App;
use crate::i18n::i18n;
use log::error;

use crate::actions::AppState;
use crate::backend::remove_matrix_access_token_if_present;
use crate::error::BKError;

pub fn dispatch_error(error: BKError) {
    match error {
        BKError::ChangePasswordError(err) => {
            let error = i18n("Couldn’t change the password");
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(show_password_error_dialog, (error));
        }
        BKError::NewRoomError(err) => {
            let err_str = format!("{:?}", err);
            error!(
                "{}",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );

            let error = i18n("Can’t create the room, try again");
            let state = AppState::NoRoom;
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
        BKError::AttachedFileError(err) => {
            let err_str = format!("{:?}", err);
            error!(
                "attaching {}: retrying send",
                remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
            );
            APPOP!(retry_send);
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
