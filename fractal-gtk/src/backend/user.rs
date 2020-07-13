use fractal_api::identifiers::UserId;
use fractal_api::reqwest::Error as ReqwestError;
use fractal_api::url::{ParseError as UrlError, Url};
use std::fs;
use std::io::Error as IoError;

use super::MediaError;
use crate::actions::global::activate_action;
use crate::backend::ThreadPool;
use crate::backend::HTTP_CLIENT;
use crate::cache::CacheMap;
use crate::util::cache_dir_path;
use crate::util::ResultExpectLog;
use log::error;
use std::convert::TryInto;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::types::Member;
use fractal_api::identity::r0::association::msisdn::submit_token::request as submit_phone_token_req;
use fractal_api::identity::r0::association::msisdn::submit_token::Body as SubmitPhoneTokenBody;
use fractal_api::identity::r0::association::msisdn::submit_token::Response as SubmitPhoneTokenResponse;
use fractal_api::r0::account::change_password::request as change_password_req;
use fractal_api::r0::account::change_password::Body as ChangePasswordBody;
use fractal_api::r0::account::change_password::Parameters as ChangePasswordParameters;
use fractal_api::r0::account::deactivate::request as deactivate;
use fractal_api::r0::account::deactivate::Body as DeactivateBody;
use fractal_api::r0::account::deactivate::Parameters as DeactivateParameters;
use fractal_api::r0::account::AuthenticationData;
use fractal_api::r0::account::Identifier;
use fractal_api::r0::account::UserIdentifier;
use fractal_api::r0::contact::create::request as create_contact;
use fractal_api::r0::contact::create::Body as AddThreePIDBody;
use fractal_api::r0::contact::create::Parameters as AddThreePIDParameters;
use fractal_api::r0::contact::delete::request as delete_contact;
use fractal_api::r0::contact::delete::Body as DeleteThreePIDBody;
use fractal_api::r0::contact::delete::Parameters as DeleteThreePIDParameters;
use fractal_api::r0::contact::get_identifiers::request as get_identifiers;
use fractal_api::r0::contact::get_identifiers::Parameters as ThirdPartyIDParameters;
use fractal_api::r0::contact::get_identifiers::Response as ThirdPartyIDResponse;
use fractal_api::r0::contact::get_identifiers::ThirdPartyIdentifier;
use fractal_api::r0::contact::request_verification_token_email::request as request_contact_verification_token_email;
use fractal_api::r0::contact::request_verification_token_email::Body as EmailTokenBody;
use fractal_api::r0::contact::request_verification_token_email::Parameters as EmailTokenParameters;
use fractal_api::r0::contact::request_verification_token_email::Response as EmailTokenResponse;
use fractal_api::r0::contact::request_verification_token_msisdn::request as request_contact_verification_token_msisdn;
use fractal_api::r0::contact::request_verification_token_msisdn::Body as PhoneTokenBody;
use fractal_api::r0::contact::request_verification_token_msisdn::Parameters as PhoneTokenParameters;
use fractal_api::r0::contact::request_verification_token_msisdn::Response as PhoneTokenResponse;
use fractal_api::r0::media::create_content::request as create_content;
use fractal_api::r0::media::create_content::Parameters as CreateContentParameters;
use fractal_api::r0::media::create_content::Response as CreateContentResponse;
use fractal_api::r0::profile::get_display_name::request as get_display_name;
use fractal_api::r0::profile::get_display_name::Parameters as GetDisplayNameParameters;
use fractal_api::r0::profile::get_display_name::Response as GetDisplayNameResponse;
use fractal_api::r0::profile::get_profile::request as get_profile;
use fractal_api::r0::profile::get_profile::Response as GetProfileResponse;
use fractal_api::r0::profile::set_avatar_url::request as set_avatar_url;
use fractal_api::r0::profile::set_avatar_url::Body as SetAvatarUrlBody;
use fractal_api::r0::profile::set_avatar_url::Parameters as SetAvatarUrlParameters;
use fractal_api::r0::profile::set_display_name::request as set_display_name;
use fractal_api::r0::profile::set_display_name::Body as SetDisplayNameBody;
use fractal_api::r0::profile::set_display_name::Parameters as SetDisplayNameParameters;
use fractal_api::r0::search::user::request as user_directory;
use fractal_api::r0::search::user::Body as UserDirectoryBody;
use fractal_api::r0::search::user::Parameters as UserDirectoryParameters;
use fractal_api::r0::search::user::Response as UserDirectoryResponse;
use fractal_api::r0::AccessToken;
use fractal_api::r0::Medium;
use fractal_api::r0::ThreePIDCredentials;

use super::{dw_media, ContentType};

use super::{remove_matrix_access_token_if_present, HandleError};
use crate::app::App;
use crate::i18n::i18n;
use crate::APPOP;

pub type UserInfo = (String, String);

#[derive(Debug)]
pub struct NameError(ReqwestError);

impl From<ReqwestError> for NameError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for NameError {}

pub fn get_username(
    base: Url,
    access_token: AccessToken,
    uid: UserId,
) -> Result<Option<String>, NameError> {
    let params = GetDisplayNameParameters { access_token };
    let request = get_display_name(base, &params, &uid)?;
    let response: GetDisplayNameResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    Ok(response.displayname)
}

// FIXME: This function manages errors *really* wrong and isn't more async
// than the normal function. It should be removed.
pub fn get_username_async(base: Url, access_token: AccessToken, uid: UserId) -> String {
    let params = GetDisplayNameParameters { access_token };

    get_display_name(base, &params, &uid)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()
                .execute(request)?
                .json::<GetDisplayNameResponse>()
        })
        .ok()
        .and_then(|response| response.displayname)
        .unwrap_or_else(|| uid.to_string())
}

#[derive(Debug)]
pub struct SetUserNameError(ReqwestError);

impl From<ReqwestError> for SetUserNameError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for SetUserNameError {}

pub fn set_username(
    base: Url,
    access_token: AccessToken,
    uid: UserId,
    username: String,
) -> Result<String, SetUserNameError> {
    let params = SetDisplayNameParameters { access_token };
    let body = SetDisplayNameBody {
        displayname: Some(username.clone()),
    };

    let request = set_display_name(base, &params, &body, &uid)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(username)
}

#[derive(Debug)]
pub struct GetThreePIDError;

impl From<ReqwestError> for GetThreePIDError {
    fn from(_: ReqwestError) -> Self {
        Self
    }
}

impl HandleError for GetThreePIDError {
    fn handle_error(&self) {
        let error = i18n("Sorry, account settings can’t be loaded.");
        APPOP!(show_load_settings_error_dialog, (error));
        let ctx = glib::MainContext::default();
        ctx.invoke(move || {
            activate_action("app", "back");
        })
    }
}

pub fn get_threepid(
    base: Url,
    access_token: AccessToken,
) -> Result<Vec<ThirdPartyIdentifier>, GetThreePIDError> {
    let params = ThirdPartyIDParameters { access_token };

    let request = get_identifiers(base, &params)?;
    let response: ThirdPartyIDResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    Ok(response.threepids)
}

#[derive(Debug)]
pub enum GetTokenEmailError {
    IdentityServerUrl(UrlError),
    Reqwest(ReqwestError),
    TokenUsed,
    Denied,
}

impl From<ReqwestError> for GetTokenEmailError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl HandleError for GetTokenEmailError {
    fn handle_error(&self) {
        match self {
            Self::TokenUsed => {
                let error = i18n("Email is already in use");
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::Denied => {
                let error = i18n("Please enter a valid email address.");
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::Reqwest(err) => {
                let error = i18n("Couldn’t add the email address.");
                let err_str = format!("{:?}", err);
                error!(
                    "{}",
                    remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
                );
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::IdentityServerUrl(err) => {
                let error = i18n("The identity server is invalid.");
                error!("The identity server is invalid: {:?}", err);
                APPOP!(show_error_dialog_in_settings, (error));
            }
        }
    }
}

pub fn get_email_token(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    email: String,
    client_secret: String,
) -> Result<(String, String), GetTokenEmailError> {
    use EmailTokenResponse::*;

    let params = EmailTokenParameters { access_token };
    let body = EmailTokenBody {
        id_server: identity
            .try_into()
            .map_err(GetTokenEmailError::IdentityServerUrl)?,
        client_secret: client_secret.clone(),
        email,
        send_attempt: 1,
        next_link: None,
    };

    let request = request_contact_verification_token_email(base, &params, &body)?;

    match HTTP_CLIENT
        .get_client()
        .execute(request)?
        .json::<EmailTokenResponse>()?
    {
        Passed(info) => Ok((info.sid, client_secret)),
        Failed(info) if info.errcode == "M_THREEPID_IN_USE" => Err(GetTokenEmailError::TokenUsed),
        Failed(_) => Err(GetTokenEmailError::Denied),
    }
}

#[derive(Debug)]
pub enum GetTokenPhoneError {
    IdentityServerUrl(UrlError),
    Reqwest(ReqwestError),
    TokenUsed,
    Denied,
}

impl From<ReqwestError> for GetTokenPhoneError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl HandleError for GetTokenPhoneError {
    fn handle_error(&self) {
        match self {
            Self::TokenUsed => {
                let error = i18n("Phone number is already in use");
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::Denied => {
                let error = i18n(
                    "Please enter your phone number in the format: \n + your country code and your phone number.",
                );
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::Reqwest(err) => {
                let error = i18n("Couldn’t add the phone number.");
                let err_str = format!("{:?}", err);
                error!(
                    "{}",
                    remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
                );
                APPOP!(show_error_dialog_in_settings, (error));
            }
            Self::IdentityServerUrl(err) => {
                let error = i18n("The identity server is invalid.");
                error!("The identity server is invalid: {:?}", err);
                APPOP!(show_error_dialog_in_settings, (error));
            }
        }
    }
}

pub fn get_phone_token(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    phone_number: String,
    client_secret: String,
) -> Result<(String, String), GetTokenPhoneError> {
    use PhoneTokenResponse::*;

    let params = PhoneTokenParameters { access_token };
    let body = PhoneTokenBody {
        id_server: identity
            .try_into()
            .map_err(GetTokenPhoneError::IdentityServerUrl)?,
        client_secret: client_secret.clone(),
        phone_number,
        country: String::new(),
        send_attempt: 1,
        next_link: None,
    };

    let request = request_contact_verification_token_msisdn(base, &params, &body)?;

    match HTTP_CLIENT
        .get_client()
        .execute(request)?
        .json::<PhoneTokenResponse>()?
    {
        Passed(info) => Ok((info.sid, client_secret)),
        Failed(info) if info.errcode == "M_THREEPID_IN_USE" => Err(GetTokenPhoneError::TokenUsed),
        Failed(_) => Err(GetTokenPhoneError::Denied),
    }
}

#[derive(Debug)]
pub enum AddedToFavError {
    IdentityServerUrl(UrlError),
    Reqwest(ReqwestError),
}

impl From<ReqwestError> for AddedToFavError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl HandleError for AddedToFavError {}

pub fn add_threepid(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    client_secret: String,
    sid: String,
) -> Result<(), AddedToFavError> {
    let params = AddThreePIDParameters { access_token };
    let body = AddThreePIDBody {
        three_pid_creds: ThreePIDCredentials {
            id_server: identity
                .try_into()
                .map_err(AddedToFavError::IdentityServerUrl)?,
            sid,
            client_secret,
        },
        bind: true,
    };

    let request = create_contact(base, &params, &body)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct SubmitPhoneTokenError(ReqwestError);

impl From<ReqwestError> for SubmitPhoneTokenError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for SubmitPhoneTokenError {}

pub fn submit_phone_token(
    base: Url,
    client_secret: String,
    sid: String,
    token: String,
) -> Result<(Option<String>, String), SubmitPhoneTokenError> {
    let body = SubmitPhoneTokenBody {
        sid: sid.clone(),
        client_secret: client_secret.clone(),
        token,
    };

    let request = submit_phone_token_req(base, &body)?;
    let response: SubmitPhoneTokenResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    Ok((Some(sid).filter(|_| response.success), client_secret))
}

#[derive(Debug)]
pub struct DeleteThreePIDError(ReqwestError);

impl From<ReqwestError> for DeleteThreePIDError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for DeleteThreePIDError {}

pub fn delete_three_pid(
    base: Url,
    access_token: AccessToken,
    medium: Medium,
    address: String,
) -> Result<(), DeleteThreePIDError> {
    let params = DeleteThreePIDParameters { access_token };
    let body = DeleteThreePIDBody { address, medium };

    let request = delete_contact(base, &params, &body)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct ChangePasswordError(ReqwestError);

impl From<ReqwestError> for ChangePasswordError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for ChangePasswordError {
    fn handle_error(&self) {
        let error = i18n("Couldn’t change the password");
        let err_str = format!("{:?}", self);
        error!(
            "{}",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );
        APPOP!(show_password_error_dialog, (error));
    }
}

pub fn change_password(
    base: Url,
    access_token: AccessToken,
    user: String,
    old_password: String,
    new_password: String,
) -> Result<(), ChangePasswordError> {
    let params = ChangePasswordParameters { access_token };
    let body = ChangePasswordBody {
        new_password,
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password: old_password,
            session: None,
        }),
    };

    let request = change_password_req(base, &params, &body)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct AccountDestructionError(ReqwestError);

impl From<ReqwestError> for AccountDestructionError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for AccountDestructionError {
    fn handle_error(&self) {
        let error = i18n("Couldn’t delete the account");
        let err_str = format!("{:?}", self.0);
        error!(
            "{}",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );
        APPOP!(show_error_dialog_in_settings, (error));
    }
}

pub fn account_destruction(
    base: Url,
    access_token: AccessToken,
    user: String,
    password: String,
) -> Result<(), AccountDestructionError> {
    let params = DeactivateParameters { access_token };
    let body = DeactivateBody {
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password,
            session: None,
        }),
    };

    let request = deactivate(base, &params, &body)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub enum SetUserAvatarError {
    Io(IoError),
    Reqwest(ReqwestError),
}

impl From<IoError> for SetUserAvatarError {
    fn from(err: IoError) -> Self {
        Self::Io(err)
    }
}

impl From<ReqwestError> for SetUserAvatarError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl HandleError for SetUserAvatarError {}

pub fn set_user_avatar(
    base: Url,
    access_token: AccessToken,
    uid: UserId,
    avatar: PathBuf,
) -> Result<PathBuf, SetUserAvatarError> {
    let params_upload = CreateContentParameters {
        access_token: access_token.clone(),
        filename: None,
    };

    let contents = fs::read(&avatar)?;
    let request = create_content(base.clone(), &params_upload, contents)?;
    let upload_response: CreateContentResponse =
        HTTP_CLIENT.get_client().execute(request)?.json()?;

    let params_avatar = SetAvatarUrlParameters { access_token };
    let body = SetAvatarUrlBody {
        avatar_url: Some(upload_response.content_uri),
    };

    let request = set_avatar_url(base, &params_avatar, &body, &uid)?;
    HTTP_CLIENT.get_client().execute(request)?;

    Ok(avatar)
}

pub fn get_user_info_async(
    thread_pool: ThreadPool,
    user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
    baseu: Url,
    uid: UserId,
    tx: Sender<UserInfo>,
) {
    if let Some(info) = user_info_cache.lock().unwrap().get(&uid).cloned() {
        thread::spawn(move || {
            tx.send(info).expect_log("Connection closed");
        });
        return;
    }

    thread_pool.run(move || {
        let info = get_user_avatar(baseu, &uid);

        if let Ok(ref i0) = info {
            user_info_cache.lock().unwrap().insert(uid, i0.clone());
        }

        tx.send(info.unwrap_or_default())
            .expect_log("Connection closed");
    });
}

#[derive(Debug)]
pub struct UserSearchError(ReqwestError);

impl From<ReqwestError> for UserSearchError {
    fn from(err: ReqwestError) -> Self {
        Self(err)
    }
}

impl HandleError for UserSearchError {}

pub fn search(
    base: Url,
    access_token: AccessToken,
    search_term: String,
) -> Result<Vec<Member>, UserSearchError> {
    let params = UserDirectoryParameters { access_token };
    let body = UserDirectoryBody {
        search_term,
        ..Default::default()
    };

    let request = user_directory(base, &params, &body)?;
    let response: UserDirectoryResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    Ok(response.results.into_iter().map(Into::into).collect())
}

#[derive(Debug)]
pub enum GetUserAvatarError {
    Reqwest(ReqwestError),
    Download(MediaError),
}

impl From<ReqwestError> for GetUserAvatarError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl HandleError for GetUserAvatarError {}

pub fn get_user_avatar(
    base: Url,
    user_id: &UserId,
) -> Result<(String, String), GetUserAvatarError> {
    let request = get_profile(base.clone(), user_id)?;
    let response: GetProfileResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    let name = response
        .displayname
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| user_id.to_string());

    let img = response
        .avatar_url
        .map(|url| {
            let dest = cache_dir_path(None, &user_id.to_string())
                .map_err(MediaError::InvalidDownloadPath)?;
            dw_media(
                base,
                url.as_str(),
                ContentType::default_thumbnail(),
                Some(dest),
            )
        })
        .unwrap_or_else(|| Ok(Default::default()))
        .map_err(GetUserAvatarError::Download)?;

    Ok((name, img))
}
