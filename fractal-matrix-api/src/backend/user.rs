use std::fs;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::util::encode_uid;
use crate::util::get_user_avatar;
use crate::util::get_user_avatar_img;
use crate::util::semaphore;
use crate::util::HTTP_CLIENT;
use reqwest::header::HeaderValue;
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

pub fn get_username(bk: &Backend) {
    let tx = bk.tx.clone();
    let uid = bk.data.lock().unwrap().user_id.clone();
    let base = bk.get_base_url();

    thread::spawn(move || {
        let query = get_display_name(base, &encode_uid(&uid))
            .and_then(|request| {
                HTTP_CLIENT
                    .execute(request)?
                    .json::<GetDisplayNameResponse>()
            })
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let name = response.displayname.unwrap_or(uid);
                let _ = tx.send(BKResponse::Name(name));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::UserNameError(err));
            }
        }
    });
}

// FIXME: This function manages errors *really* wrong and isn't more async
// than the normal function. It should be removed.
pub fn get_username_async(bk: &Backend, uid: String, tx: Sender<String>) {
    let base = bk.get_base_url();

    thread::spawn(move || {
        let query = get_display_name(base, &encode_uid(&uid)).and_then(|request| {
            HTTP_CLIENT
                .execute(request)?
                .json::<GetDisplayNameResponse>()
        });

        match query {
            Ok(response) => {
                let name = response.displayname.unwrap_or(uid);
                let _ = tx.send(name);
            }
            Err(_) => {
                let _ = tx.send(uid);
            }
        }
    });
}

pub fn set_username(bk: &Backend, name: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let uid = bk.data.lock().unwrap().user_id.clone();
    let params = SetDisplayNameParameters { access_token };
    let body = SetDisplayNameBody {
        displayname: Some(name.clone()),
    };

    thread::spawn(move || {
        let query = set_display_name(base, &params, &body, &encode_uid(&uid))
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::SetUserName(name));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::SetUserNameError(err));
            }
        }
    });
}

pub fn get_threepid(bk: &Backend) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = ThirdPartyIDParameters { access_token };

    thread::spawn(move || {
        let query = get_identifiers(base, &params)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<ThirdPartyIDResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let _ = tx.send(BKResponse::GetThreePID(response.threepids));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::GetThreePIDError(err));
            }
        }
    });
}

pub fn get_email_token(bk: &Backend, identity: String, email: String, client_secret: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = EmailTokenParameters { access_token };
    let body = EmailTokenBody {
        id_server: identity[8..].into(),
        client_secret: client_secret.clone(),
        email,
        send_attempt: 1,
        next_link: None,
    };

    thread::spawn(move || {
        let query = request_contact_verification_token_email(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<EmailTokenResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let _ = tx.send(BKResponse::GetTokenEmail(response.sid, client_secret));
            }
            Err(Error::MatrixError(ref js))
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                let _ = tx.send(BKResponse::GetTokenEmailUsed);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::GetTokenEmailError(err));
            }
        }
    });
}

pub fn get_phone_token(bk: &Backend, identity: String, phone: String, client_secret: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = PhoneTokenParameters { access_token };
    let body = PhoneTokenBody {
        id_server: identity[8..].into(),
        client_secret: client_secret.clone(),
        phone_number: phone,
        country: String::new(),
        send_attempt: 1,
        next_link: None,
    };

    thread::spawn(move || {
        let query = request_contact_verification_token_msisdn(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<PhoneTokenResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let _ = tx.send(BKResponse::GetTokenPhone(response.sid, client_secret));
            }
            Err(Error::MatrixError(ref js))
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                let _ = tx.send(BKResponse::GetTokenPhoneUsed);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::GetTokenPhoneError(err));
            }
        }
    });
}

pub fn add_threepid(bk: &Backend, identity: String, client_secret: String, sid: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = AddThreePIDParameters { access_token };
    let body = AddThreePIDBody {
        three_pid_creds: ThreePIDCredentials {
            id_server: identity[8..].into(),
            sid: sid.clone(),
            client_secret,
        },
        bind: true,
    };

    thread::spawn(move || {
        let query = create_contact(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::AddThreePID(sid));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::AddThreePIDError(err));
            }
        }
    });
}

pub fn submit_phone_token(bk: &Backend, client_secret: String, sid: String, token: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let body = SubmitPhoneTokenBody {
        sid: sid.clone(),
        client_secret: client_secret.clone(),
        token,
    };

    thread::spawn(move || {
        let query = submit_phone_token_req(base, &body)
            .and_then(|request| {
                HTTP_CLIENT
                    .execute(request)?
                    .json::<SubmitPhoneTokenResponse>()
            })
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let result = Some(sid).filter(|_| response.success);
                let _ = tx.send(BKResponse::SubmitPhoneToken(result, client_secret));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::SubmitPhoneTokenError(err));
            }
        }
    });
}

pub fn delete_three_pid(bk: &Backend, medium: Medium, address: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = DeleteThreePIDParameters { access_token };
    let body = DeleteThreePIDBody { address, medium };

    thread::spawn(move || {
        let query = delete_contact(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::DeleteThreePID);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::DeleteThreePIDError(err));
            }
        }
    });
}

pub fn change_password(bk: &Backend, user: String, old_password: String, new_password: String) {
    let tx = bk.tx.clone();

    let access_token = bk.data.lock().unwrap().access_token.clone();
    let base = bk.get_base_url();
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
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::ChangePassword);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::ChangePasswordError(err));
            }
        }
    });
}

pub fn account_destruction(bk: &Backend, user: String, password: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
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
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::AccountDestruction);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::AccountDestructionError(err));
            }
        }
    });
}

pub fn get_avatar(bk: &Backend) {
    let base = bk.get_base_url();
    let userid = bk.data.lock().unwrap().user_id.clone();

    let tx = bk.tx.clone();
    thread::spawn(move || match get_user_avatar(&base, &userid) {
        Ok((_, fname)) => {
            let _ = tx.send(BKResponse::Avatar(fname));
        }
        Err(err) => {
            let _ = tx.send(BKResponse::AvatarError(err));
        }
    });
}

pub fn get_avatar_async(bk: &Backend, member: Option<Member>, tx: Sender<String>) {
    if let Some(member) = member {
        let base = bk.get_base_url();
        let uid = member.uid.clone();
        let avatar = member.avatar.clone().unwrap_or_default();

        semaphore(
            bk.limit_threads.clone(),
            move || match get_user_avatar_img(&base, &uid, &avatar) {
                Ok(fname) => {
                    let _ = tx.send(fname);
                }
                Err(_) => {
                    let _ = tx.send(Default::default());
                }
            },
        );
    } else {
        let _ = tx.send(Default::default());
    }
}

pub fn set_user_avatar(bk: &Backend, avatar: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let id = bk.data.lock().unwrap().user_id.clone();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params_upload = CreateContentParameters {
        access_token: access_token.clone(),
        filename: None,
    };

    thread::spawn(move || {
        let query = fs::read(&avatar).map_err(Into::into).and_then(|contents| {
            let (mime, _) = gio::content_type_guess(None, &contents);
            let mime_value = HeaderValue::from_str(&mime).or(Err(Error::BackendError))?;
            let upload_response =
                create_content(base.clone(), &params_upload, contents, Some(mime_value)).and_then(
                    |request| {
                        HTTP_CLIENT
                            .execute(request)?
                            .json::<CreateContentResponse>()
                    },
                )?;

            let params_avatar = SetAvatarUrlParameters { access_token };
            let body = SetAvatarUrlBody {
                avatar_url: Some(upload_response.content_uri),
            };

            set_avatar_url(base, &params_avatar, &body, &encode_uid(&id))
                .and_then(|request| HTTP_CLIENT.execute(request))
                .map_err(Into::into)
        });

        match query {
            Ok(_) => {
                let _ = tx.send(BKResponse::SetUserAvatar(avatar));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::SetUserAvatarError(err));
            }
        }
    });
}

pub fn get_user_info_async(bk: &mut Backend, uid: &str, tx: Option<Sender<(String, String)>>) {
    let baseu = bk.get_base_url();

    let u = uid.to_string();

    if let Some(info) = bk.user_info_cache.get(&u) {
        if let Some(tx) = tx.clone() {
            let info = info.clone();
            thread::spawn(move || {
                let i = info.lock().unwrap().clone();
                tx.send(i).unwrap();
            });
        }
        return;
    }

    let info = Arc::new(Mutex::new((String::new(), String::new())));
    let cache_key = u.clone();
    let cache_value = info.clone();

    semaphore(bk.limit_threads.clone(), move || {
        let i0 = info.lock();
        match get_user_avatar(&baseu, &u) {
            Ok(info) => {
                if let Some(tx) = tx.clone() {
                    tx.send(info.clone()).unwrap();
                    let mut i = i0.unwrap();
                    i.0 = info.0;
                    i.1 = info.1;
                }
            }
            Err(_) => {
                if let Some(tx) = tx.clone() {
                    tx.send((String::new(), String::new())).unwrap();
                }
            }
        };
    });

    bk.user_info_cache.insert(cache_key, cache_value);
}

pub fn search(bk: &Backend, search_term: String) {
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let access_token = bk.data.lock().unwrap().access_token.clone();
    let params = UserDirectoryParameters { access_token };
    let body = UserDirectoryBody {
        search_term,
        ..Default::default()
    };

    thread::spawn(move || {
        let query = user_directory(base, &params, &body)
            .and_then(|request| {
                HTTP_CLIENT
                    .execute(request)?
                    .json::<UserDirectoryResponse>()
            })
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let users = response.results.into_iter().map(Into::into).collect();
                let _ = tx.send(BKResponse::UserSearch(users));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::CommandError(err));
            }
        }
    });
}
