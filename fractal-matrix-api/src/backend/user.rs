use std::fs;
use url::Url;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::encode_uid;
use crate::util::get_user_avatar;
use crate::util::semaphore;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;
use reqwest::header::HeaderValue;
use std::convert::TryInto;
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
use crate::r0::contact::request_verification_token_email::request as request_contact_verification_token_email;
use crate::r0::contact::request_verification_token_email::Body as EmailTokenBody;
use crate::r0::contact::request_verification_token_email::Parameters as EmailTokenParameters;
use crate::r0::contact::request_verification_token_email::Response as EmailTokenResponse;
use crate::r0::contact::request_verification_token_msisdn::request as request_contact_verification_token_msisdn;
use crate::r0::contact::request_verification_token_msisdn::Body as PhoneTokenBody;
use crate::r0::contact::request_verification_token_msisdn::Parameters as PhoneTokenParameters;
use crate::r0::contact::request_verification_token_msisdn::Response as PhoneTokenResponse;
use crate::r0::media::create::request as create_content;
use crate::r0::media::create::Parameters as CreateContentParameters;
use crate::r0::media::create::Response as CreateContentResponse;
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
use crate::r0::Medium;
use crate::r0::ThreePIDCredentials;
use crate::types::Member;

pub fn get_username(bk: &Backend, base: Url) {
    let tx = bk.tx.clone();
    let uid = bk.data.lock().unwrap().user_id.clone();

    thread::spawn(move || {
        let query = get_display_name(base, &encode_uid(&uid))
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<GetDisplayNameResponse>()
                    .map_err(Into::into)
            })
            .map(|response| response.displayname.unwrap_or(uid));

        tx.send(BKResponse::Name(query))
            .expect_log("Connection closed");
    });
}

// FIXME: This function manages errors *really* wrong and isn't more async
// than the normal function. It should be removed.
pub fn get_username_async(base: Url, uid: String, tx: Sender<String>) {
    thread::spawn(move || {
        let query = get_display_name(base, &encode_uid(&uid))
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
            .unwrap_or(uid);

        tx.send(query).expect_log("Connection closed");
    });
}

pub fn set_username(bk: &Backend, base: Url, name: String) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let uid = bk.data.lock().unwrap().user_id.clone();
    let params = SetDisplayNameParameters { access_token };
    let body = SetDisplayNameBody {
        displayname: Some(name.clone()),
    };

    thread::spawn(move || {
        let query = set_display_name(base, &params, &body, &encode_uid(&uid))
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(name));

        tx.send(BKResponse::SetUserName(query))
            .expect_log("Connection closed");
    });
}

pub fn get_threepid(bk: &Backend, base: Url) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = ThirdPartyIDParameters { access_token };

    thread::spawn(move || {
        let query = get_identifiers(base, &params)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<ThirdPartyIDResponse>()
                    .map_err(Into::into)
            })
            .map(|response| response.threepids);

        tx.send(BKResponse::GetThreePID(query))
            .expect_log("Connection closed");
    });
}

pub fn get_email_token(
    bk: &Backend,
    base: Url,
    identity: String,
    email: String,
    client_secret: String,
) -> Result<(), Error> {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = EmailTokenParameters { access_token };
    let body = EmailTokenBody {
        id_server: Url::parse(&identity)?.try_into()?,
        client_secret: client_secret.clone(),
        email,
        send_attempt: 1,
        next_link: None,
    };

    thread::spawn(move || {
        let query = request_contact_verification_token_email(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<EmailTokenResponse>()
                    .map_err(Into::into)
            })
            .and_then(|response| match response {
                EmailTokenResponse::Passed(info) => Ok(info.sid),
                EmailTokenResponse::Failed(info) => {
                    if info.errcode == "M_THREEPID_IN_USE" {
                        Err(Error::TokenUsed)
                    } else {
                        Err(Error::Denied)
                    }
                }
            })
            .map(|response| (response, client_secret));

        tx.send(BKResponse::GetTokenEmail(query))
            .expect_log("Connection closed");
    });

    Ok(())
}

pub fn get_phone_token(
    bk: &Backend,
    base: Url,
    identity: String,
    phone: String,
    client_secret: String,
) -> Result<(), Error> {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = PhoneTokenParameters { access_token };
    let body = PhoneTokenBody {
        id_server: Url::parse(&identity)?.try_into()?,
        client_secret: client_secret.clone(),
        phone_number: phone,
        country: String::new(),
        send_attempt: 1,
        next_link: None,
    };

    thread::spawn(move || {
        let query = request_contact_verification_token_msisdn(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<PhoneTokenResponse>()
                    .map_err(Into::into)
            })
            .and_then(|response| match response {
                PhoneTokenResponse::Passed(info) => Ok(info.sid),
                PhoneTokenResponse::Failed(info) => {
                    if info.errcode == "M_THREEPID_IN_USE" {
                        Err(Error::TokenUsed)
                    } else {
                        Err(Error::Denied)
                    }
                }
            })
            .map(|response| (response, client_secret));

        tx.send(BKResponse::GetTokenPhone(query))
            .expect_log("Connection closed");
    });

    Ok(())
}

pub fn add_threepid(
    bk: &Backend,
    base: Url,
    identity: String,
    client_secret: String,
    sid: String,
) -> Result<(), Error> {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = AddThreePIDParameters { access_token };
    let body = AddThreePIDBody {
        three_pid_creds: ThreePIDCredentials {
            id_server: Url::parse(&identity)?.try_into()?,
            sid: sid.clone(),
            client_secret,
        },
        bind: true,
    };

    thread::spawn(move || {
        let query = create_contact(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        tx.send(BKResponse::AddThreePID(query))
            .expect_log("Connection closed");
    });

    Ok(())
}

pub fn submit_phone_token(
    bk: &Backend,
    base: Url,
    client_secret: String,
    sid: String,
    token: String,
) {
    let tx = bk.tx.clone();

    let body = SubmitPhoneTokenBody {
        sid: sid.clone(),
        client_secret: client_secret.clone(),
        token,
    };

    thread::spawn(move || {
        let query = submit_phone_token_req(base, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<SubmitPhoneTokenResponse>()
                    .map_err(Into::into)
            })
            .map(|response| (Some(sid).filter(|_| response.success), client_secret));

        tx.send(BKResponse::SubmitPhoneToken(query))
            .expect_log("Connection closed");
    });
}

pub fn delete_three_pid(bk: &Backend, base: Url, medium: Medium, address: String) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = DeleteThreePIDParameters { access_token };
    let body = DeleteThreePIDBody { address, medium };

    thread::spawn(move || {
        let query = delete_contact(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        tx.send(BKResponse::DeleteThreePID(query))
            .expect_log("Connection closed");
    });
}

pub fn change_password(
    bk: &Backend,
    base: Url,
    user: String,
    old_password: String,
    new_password: String,
) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = ChangePasswordParameters { access_token };
    let body = ChangePasswordBody {
        new_password,
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password: old_password,
            session: None,
        }),
    };

    thread::spawn(move || {
        let query = change_password_req(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        tx.send(BKResponse::ChangePassword(query))
            .expect_log("Connection closed");
    });
}

pub fn account_destruction(bk: &Backend, base: Url, user: String, password: String) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = DeactivateParameters { access_token };
    let body = DeactivateBody {
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password,
            session: None,
        }),
    };

    thread::spawn(move || {
        let query = deactivate(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        tx.send(BKResponse::AccountDestruction(query))
            .expect_log("Connection closed");
    });
}

pub fn get_avatar(bk: &Backend, base: Url) {
    let userid = bk.data.lock().unwrap().user_id.clone();

    let tx = bk.tx.clone();
    thread::spawn(move || {
        let query = get_user_avatar(&base, &userid).map(|(_, fname)| fname);
        tx.send(BKResponse::Avatar(query))
            .expect_log("Connection closed");
    });
}

pub fn get_avatar_async(bk: &Backend, base: Url, member: Option<Member>, tx: Sender<String>) {
    if let Some(member) = member {
        let uid = member.uid.clone();
        let avatar = member.avatar.clone().unwrap_or_default();

        semaphore(bk.limit_threads.clone(), move || {
            let fname = get_user_avatar_img(&base, &uid, &avatar).unwrap_or_default();
            tx.send(fname).expect_log("Connection closed");
        });
    } else {
        tx.send(Default::default()).expect_log("Connection closed");
    }
}

pub fn set_user_avatar(bk: &Backend, base: Url, avatar: String) {
    let tx = bk.tx.clone();

    let id = bk.data.lock().unwrap().user_id.clone();
    let access_token = bk.get_access_token();
    let params_upload = CreateContentParameters {
        access_token: access_token.clone(),
        filename: None,
    };

    thread::spawn(move || {
        let query = fs::read(&avatar)
            .map_err(Into::into)
            .and_then(|contents| {
                let (mime, _) = gio::content_type_guess(None, &contents);
                let mime_value = HeaderValue::from_str(&mime).or(Err(Error::BackendError))?;
                let upload_response =
                    create_content(base.clone(), &params_upload, contents, Some(mime_value))
                        .map_err::<Error, _>(Into::into)
                        .and_then(|request| {
                            HTTP_CLIENT
                                .get_client()?
                                .execute(request)?
                                .json::<CreateContentResponse>()
                                .map_err(Into::into)
                        })?;

                let params_avatar = SetAvatarUrlParameters { access_token };
                let body = SetAvatarUrlBody {
                    avatar_url: Some(upload_response.content_uri),
                };

                set_avatar_url(base, &params_avatar, &body, &encode_uid(&id))
                    .map_err(Into::into)
                    .and_then(|request| {
                        HTTP_CLIENT
                            .get_client()?
                            .execute(request)
                            .map_err(Into::into)
                    })
            })
            .and(Ok(avatar));

        tx.send(BKResponse::SetUserAvatar(query))
            .expect_log("Connection closed");
    });
}

pub fn get_user_info_async(
    bk: &mut Backend,
    baseu: Url,
    uid: String,
    tx: Option<Sender<(String, String)>>,
) {
    if let Some(info) = bk.user_info_cache.get(&uid) {
        if let Some(tx) = tx.clone() {
            let info = info.clone();
            thread::spawn(move || {
                let i = info.lock().unwrap().clone();
                tx.send(i).expect_log("Connection closed");
            });
        }
        return;
    }

    let info: Arc<Mutex<(String, String)>> = Default::default();
    bk.user_info_cache.insert(uid.clone(), info.clone());

    semaphore(bk.limit_threads.clone(), move || {
        match (get_user_avatar(&baseu, &uid), tx) {
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

pub fn search(bk: &Backend, base: Url, search_term: String) {
    let tx = bk.tx.clone();

    let access_token = bk.get_access_token();
    let params = UserDirectoryParameters { access_token };
    let body = UserDirectoryBody {
        search_term,
        ..Default::default()
    };

    thread::spawn(move || {
        let query = user_directory(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<UserDirectoryResponse>()
                    .map_err(Into::into)
            })
            .map(|response| response.results.into_iter().map(Into::into).collect());

        tx.send(BKResponse::UserSearch(query))
            .expect_log("Connection closed");
    });
}

fn get_user_avatar_img(baseu: &Url, userid: &str, avatar: &str) -> Result<String, Error> {
    if avatar.is_empty() {
        return Ok(String::new());
    }

    let dest = cache_dir_path(None, &userid)?;
    dw_media(
        baseu,
        &avatar,
        ContentType::default_thumbnail(),
        Some(&dest),
    )
}
