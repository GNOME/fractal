use crate::backend::remove_matrix_access_token_if_present;
use crate::error::BKError;
use log::error;

pub fn dispatch_error(err: BKError) {
    let err_str = format!("{:?}", err);
    error!(
        "Query error: {}",
        remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
    );
}
