use ruma_identifiers::UserId;
use std::fs;
use url::Url;

use crate::backend::types::Backend;
use crate::error::Error;
use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::get_user_avatar;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;
use std::convert::TryInto;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::identity::r0::association::msisdn::submit_token::request as submit_phone_token_req;
use crate::identity::r0::association::msisdn::submit_token::Body as SubmitPhoneTokenBody;
use crate::identity::r0::association::msisdn::submit_token::Response as SubmitPhoneTokenResponse;
use crate::r0::account::change_password::request as change_password_req;
use crate::r0::account::change_password::Body as ChangePasswordBody;
use crate::r0::account::change_password::Parameters as ChangePasswordParameters;
use crate::r0::account::deactivate::request as deactivate;
use crate::r0::account::deactivate::Body as DeactivateBody;
use crate::r0::account::deactivate::Parameters as DeactivateParameters;
use crate::r0::account::AuthenticationData;
use crate::r0::account::Identifier;
use crate::r0::account::UserIdentifier;
use crate::r0::contact::create::request as create_contact;
use crate::r0::contact::create::Body as AddThreePIDBody;
use crate::r0::contact::create::Parameters as AddThreePIDParameters;
use crate::r0::contact::delete::request as delete_contact;
use crate::r0::contact::delete::Body as DeleteThreePIDBody;
use crate::r0::contact::delete::Parameters as DeleteThreePIDParameters;
use crate::r0::contact::get_identifiers::request as get_identifiers;
use crate::r0::contact::get_identifiers::Parameters as ThirdPartyIDParameters;
use crate::r0::contact::get_identifiers::Response as ThirdPartyIDResponse;
use crate::r0::contact::get_identifiers::ThirdPartyIdentifier;
use crate::r0::contact::request_verification_token_email::request as request_contact_verification_token_email;
use crate::r0::contact::request_verification_token_email::Body as EmailTokenBody;
use crate::r0::contact::request_verification_token_email::Parameters as EmailTokenParameters;
use crate::r0::contact::request_verification_token_email::Response as EmailTokenResponse;
use crate::r0::contact::request_verification_token_msisdn::request as request_contact_verification_token_msisdn;
use crate::r0::contact::request_verification_token_msisdn::Body as PhoneTokenBody;
use crate::r0::contact::request_verification_token_msisdn::Parameters as PhoneTokenParameters;
use crate::r0::contact::request_verification_token_msisdn::Response as PhoneTokenResponse;
use crate::r0::media::create_content::request as create_content;
use crate::r0::media::create_content::Parameters as CreateContentParameters;
use crate::r0::media::create_content::Response as CreateContentResponse;
use crate::r0::profile::get_display_name::request as get_display_name;
use crate::r0::profile::get_display_name::Response as GetDisplayNameResponse;
use crate::r0::profile::set_avatar_url::request as set_avatar_url;
use crate::r0::profile::set_avatar_url::Body as SetAvatarUrlBody;
use crate::r0::profile::set_avatar_url::Parameters as SetAvatarUrlParameters;
use crate::r0::profile::set_display_name::request as set_display_name;
use crate::r0::profile::set_display_name::Body as SetDisplayNameBody;
use crate::r0::profile::set_display_name::Parameters as SetDisplayNameParameters;
use crate::r0::search::user::request as user_directory;
use crate::r0::search::user::Body as UserDirectoryBody;
use crate::r0::search::user::Parameters as UserDirectoryParameters;
use crate::r0::search::user::Response as UserDirectoryResponse;
use crate::r0::AccessToken;
use crate::r0::Medium;
use crate::r0::ThreePIDCredentials;
use crate::types::Member;

pub fn get_username(base: Url, uid: UserId) -> Result<Option<String>, Error> {
    let request = get_display_name(base, &uid)?;
    let response: GetDisplayNameResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok(response.displayname)
}

// FIXME: This function manages errors *really* wrong and isn't more async
// than the normal function. It should be removed.
pub fn get_username_async(base: Url, uid: UserId) -> String {
    get_display_name(base, &uid)
        .map_err::<Error, _>(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<GetDisplayNameResponse>()
                .map_err(Into::into)
        })
        .ok()
        .and_then(|response| response.displayname)
        .unwrap_or_else(|| uid.to_string())
}

pub fn set_username(
    base: Url,
    access_token: AccessToken,
    uid: UserId,
    username: String,
) -> Result<String, Error> {
    let params = SetDisplayNameParameters { access_token };
    let body = SetDisplayNameBody {
        displayname: Some(username.clone()),
    };

    let request = set_display_name(base, &params, &body, &uid)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(username)
}

pub fn get_threepid(
    base: Url,
    access_token: AccessToken,
) -> Result<Vec<ThirdPartyIdentifier>, Error> {
    let params = ThirdPartyIDParameters { access_token };

    let request = get_identifiers(base, &params)?;
    let response: ThirdPartyIDResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok(response.threepids)
}

pub fn get_email_token(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    email: String,
    client_secret: String,
) -> Result<(String, String), Error> {
    use EmailTokenResponse::*;

    let params = EmailTokenParameters { access_token };
    let body = EmailTokenBody {
        id_server: identity.try_into()?,
        client_secret: client_secret.clone(),
        email,
        send_attempt: 1,
        next_link: None,
    };

    let request = request_contact_verification_token_email(base, &params, &body)?;

    match HTTP_CLIENT
        .get_client()?
        .execute(request)?
        .json::<EmailTokenResponse>()?
    {
        Passed(info) => Ok((info.sid, client_secret)),
        Failed(info) if info.errcode == "M_THREEPID_IN_USE" => Err(Error::TokenUsed),
        Failed(_) => Err(Error::Denied),
    }
}

pub fn get_phone_token(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    phone: String,
    client_secret: String,
) -> Result<(String, String), Error> {
    use PhoneTokenResponse::*;

    let params = PhoneTokenParameters { access_token };
    let body = PhoneTokenBody {
        id_server: identity.try_into()?,
        client_secret: client_secret.clone(),
        phone_number: phone,
        country: String::new(),
        send_attempt: 1,
        next_link: None,
    };

    let request = request_contact_verification_token_msisdn(base, &params, &body)?;

    match HTTP_CLIENT
        .get_client()?
        .execute(request)?
        .json::<PhoneTokenResponse>()?
    {
        Passed(info) => Ok((info.sid, client_secret)),
        Failed(info) if info.errcode == "M_THREEPID_IN_USE" => Err(Error::TokenUsed),
        Failed(_) => Err(Error::Denied),
    }
}

pub fn add_threepid(
    base: Url,
    access_token: AccessToken,
    identity: Url,
    client_secret: String,
    sid: String,
) -> Result<(), Error> {
    let params = AddThreePIDParameters { access_token };
    let body = AddThreePIDBody {
        three_pid_creds: ThreePIDCredentials {
            id_server: identity.try_into()?,
            sid,
            client_secret,
        },
        bind: true,
    };

    let request = create_contact(base, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn submit_phone_token(
    base: Url,
    client_secret: String,
    sid: String,
    token: String,
) -> Result<(Option<String>, String), Error> {
    let body = SubmitPhoneTokenBody {
        sid: sid.clone(),
        client_secret: client_secret.clone(),
        token,
    };

    let request = submit_phone_token_req(base, &body)?;
    let response: SubmitPhoneTokenResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok((Some(sid).filter(|_| response.success), client_secret))
}

pub fn delete_three_pid(
    base: Url,
    access_token: AccessToken,
    medium: Medium,
    address: String,
) -> Result<(), Error> {
    let params = DeleteThreePIDParameters { access_token };
    let body = DeleteThreePIDBody { address, medium };

    let request = delete_contact(base, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn change_password(
    base: Url,
    access_token: AccessToken,
    user: String,
    old_password: String,
    new_password: String,
) -> Result<(), Error> {
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
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn account_destruction(
    base: Url,
    access_token: AccessToken,
    user: String,
    password: String,
) -> Result<(), Error> {
    let params = DeactivateParameters { access_token };
    let body = DeactivateBody {
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password,
            session: None,
        }),
    };

    let request = deactivate(base, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn get_avatar(base: Url, userid: UserId) -> Result<PathBuf, Error> {
    get_user_avatar(base, &userid).map(|(_, fname)| fname.into())
}

pub fn get_avatar_async(bk: &Backend, base: Url, member: Option<Member>, tx: Sender<String>) {
    if let Some(member) = member {
        let uid = member.uid.clone();
        let avatar = member.avatar.unwrap_or_default();

        bk.thread_pool.run(move || {
            let fname = get_user_avatar_img(base, &uid, &avatar).unwrap_or_default();
            tx.send(fname).expect_log("Connection closed");
        });
    } else {
        tx.send(Default::default()).expect_log("Connection closed");
    }
}

pub fn set_user_avatar(
    base: Url,
    access_token: AccessToken,
    uid: UserId,
    avatar: PathBuf,
) -> Result<PathBuf, Error> {
    let params_upload = CreateContentParameters {
        access_token: access_token.clone(),
        filename: None,
    };

    let contents = fs::read(&avatar)?;
    let request = create_content(base.clone(), &params_upload, contents)?;
    let upload_response: CreateContentResponse =
        HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let params_avatar = SetAvatarUrlParameters { access_token };
    let body = SetAvatarUrlBody {
        avatar_url: Some(upload_response.content_uri),
    };

    let request = set_avatar_url(base, &params_avatar, &body, &uid)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(avatar)
}

pub fn get_user_info_async(
    bk: &mut Backend,
    baseu: Url,
    uid: UserId,
    tx: Option<Sender<(String, String)>>,
) {
    if let Some(info) = bk.user_info_cache.get(&uid).cloned() {
        if let Some(tx) = tx {
            thread::spawn(move || {
                let i = info.lock().unwrap().clone();
                tx.send(i).expect_log("Connection closed");
            });
        }
        return;
    }

    let info: Arc<Mutex<(String, String)>> = Default::default();
    bk.user_info_cache.insert(uid.clone(), info.clone());

    bk.thread_pool.run(move || {
        match (get_user_avatar(baseu, &uid), tx) {
            (Ok(i0), Some(tx)) => {
                tx.send(i0.clone()).expect_log("Connection closed");
                *info.lock().unwrap() = i0;
            }
            (Err(_), Some(tx)) => {
                tx.send(Default::default()).expect_log("Connection closed");
            }
            _ => {}
        };
    });
}

pub fn search(
    base: Url,
    access_token: AccessToken,
    search_term: String,
) -> Result<Vec<Member>, Error> {
    let params = UserDirectoryParameters { access_token };
    let body = UserDirectoryBody {
        search_term,
        ..Default::default()
    };

    let request = user_directory(base, &params, &body)?;
    let response: UserDirectoryResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok(response.results.into_iter().map(Into::into).collect())
}

fn get_user_avatar_img(baseu: Url, userid: &UserId, avatar: &str) -> Result<String, Error> {
    let dest = cache_dir_path(None, &userid.to_string())?;

    dw_media(baseu, avatar, ContentType::default_thumbnail(), Some(dest))
}
