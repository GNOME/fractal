use fractal_api::identifiers::{DeviceId, UserId};
use fractal_api::reqwest::Error as ReqwestError;
use fractal_api::url::{ParseError as UrlError, Url};

use crate::actions::AppState;
use crate::backend::HTTP_CLIENT;
use crate::globals;
use fractal_api::r0::account::login::request as login_req;
use fractal_api::r0::account::login::Auth;
use fractal_api::r0::account::login::Body as LoginBody;
use fractal_api::r0::account::login::Response as LoginResponse;
use fractal_api::r0::account::logout::request as logout_req;
use fractal_api::r0::account::logout::Parameters as LogoutParameters;
use fractal_api::r0::account::Identifier;
use fractal_api::r0::account::UserIdentifier;
use fractal_api::r0::server::domain_info::request as domain_info;
use fractal_api::r0::server::domain_info::Response as DomainInfoResponse;
use fractal_api::r0::AccessToken;
use fractal_api::r0::Medium;

use super::HandleError;
use crate::app::App;
use crate::i18n::i18n;
use crate::APPOP;

#[derive(Debug)]
pub struct LoginError;

impl From<ReqwestError> for LoginError {
    fn from(_: ReqwestError) -> Self {
        Self
    }
}

impl HandleError for LoginError {
    fn handle_error(&self) {
        let error = i18n("Can’t login, try again");
        let st = AppState::Login;
        APPOP!(show_error, (error));
        APPOP!(logout);
        APPOP!(set_state, (st));
    }
}

pub fn login(
    user: String,
    password: String,
    server: Url,
) -> Result<(UserId, AccessToken, Option<Box<DeviceId>>), LoginError> {
    let body = if globals::EMAIL_RE.is_match(&user) {
        LoginBody {
            auth: Auth::Password { password },
            identifier: Identifier::new(UserIdentifier::ThirdParty {
                medium: Medium::Email,
                address: user,
            }),
            initial_device_display_name: Some(globals::DEVICE_NAME.into()),
            device_id: None,
        }
    } else {
        LoginBody {
            auth: Auth::Password { password },
            identifier: Identifier::new(UserIdentifier::User { user }),
            initial_device_display_name: Some(globals::DEVICE_NAME.into()),
            device_id: None,
        }
    };

    let request = login_req(server, &body)?;
    let response: LoginResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    if let (Some(tk), Some(uid)) = (response.access_token, response.user_id) {
        Ok((uid, tk, response.device_id))
    } else {
        Err(LoginError)
    }
}

#[derive(Debug)]
pub struct LogoutError(ReqwestError);

impl From<ReqwestError> for LogoutError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for LogoutError {}

pub fn logout(server: Url, access_token: AccessToken) -> Result<(), LogoutError> {
    let params = LogoutParameters { access_token };

    let request = logout_req(server, &params)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub enum GetWellKnownError {
    Reqwest(ReqwestError),
    ParseUrl(UrlError),
}

impl From<ReqwestError> for GetWellKnownError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl From<UrlError> for GetWellKnownError {
    fn from(err: UrlError) -> Self {
        Self::ParseUrl(err)
    }
}

pub fn get_well_known(domain: Url) -> Result<DomainInfoResponse, GetWellKnownError> {
    let request = domain_info(domain)?;

    HTTP_CLIENT
        .get_client()
        .execute(request)?
        .json()
        .map_err(Into::into)
}
