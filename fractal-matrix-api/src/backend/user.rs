use log::info;
use serde_json::json;
use std::fs::File;
use std::io::prelude::*;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use crate::util::encode_uid;
use crate::util::get_user_avatar;
use crate::util::get_user_avatar_img;
use crate::util::json_q;
use crate::util::put_media;
use crate::util::semaphore;
use crate::util::{build_url, media_url};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use url::Url;

use crate::types::AddThreePIDRequest;
use crate::types::AuthenticationData;
use crate::types::ChangePasswordRequest;
use crate::types::DeactivateAccountRequest;
use crate::types::DeleteThreePIDRequest;
use crate::types::EmailTokenRequest;
use crate::types::GetDisplayNameResponse;
use crate::types::Identifier;
use crate::types::Medium;
use crate::types::Member;
use crate::types::PhoneTokenRequest;
use crate::types::PutDisplayNameRequest;
use crate::types::SearchUserRequest;
use crate::types::SearchUserResponse;
use crate::types::SubmitPhoneTokenRequest;
use crate::types::SubmitPhoneTokenResponse;
use crate::types::ThirdPartyIDResponse;
use crate::types::ThirdPartyTokenResponse;
use crate::types::ThreePIDCredentials;
use crate::types::UserIdentifier;

use serde_json;
use serde_json::Value as JsonValue;

pub fn get_username(bk: &Backend) -> Result<(), Error> {
    let id = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![])?;
    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<GetDisplayNameResponse>(r) {
            let name = response.displayname.unwrap_or(id);
            tx.send(BKResponse::Name(name)).unwrap();
        } else {
            tx.send(BKResponse::UserNameError(Error::BackendError))
                .unwrap();
        },
        |err| tx.send(BKResponse::UserNameError(err)).unwrap()
    );

    Ok(())
}

pub fn set_username(bk: &Backend, name: String) -> Result<(), Error> {
    let id = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![])?;

    let attrs = PutDisplayNameRequest {
        displayname: Some(name.clone()),
    };
    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize display name setting request");

    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs_json,
        |_| {
            tx.send(BKResponse::SetUserName(name)).unwrap();
        },
        |err| {
            tx.send(BKResponse::SetUserNameError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn get_threepid(bk: &Backend) -> Result<(), Error> {
    let url = bk.url(&format!("account/3pid"), vec![])?;
    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<ThirdPartyIDResponse>(r) {
            tx.send(BKResponse::GetThreePID(response.threepids))
                .unwrap();
        } else {
            tx.send(BKResponse::GetThreePIDError(Error::BackendError))
                .unwrap();
        },
        |err| tx.send(BKResponse::GetThreePIDError(err)).unwrap()
    );

    Ok(())
}

pub fn get_email_token(
    bk: &Backend,
    identity: String,
    email: String,
    client_secret: String,
) -> Result<(), Error> {
    let url = bk.url("account/3pid/email/requestToken", vec![])?;

    let attrs = EmailTokenRequest {
        id_server: identity[8..].into(),
        client_secret: client_secret.clone(),
        email: email,
        send_attempt: 1,
        next_link: None,
    };

    let attrs_json = serde_json::to_value(attrs).expect("Failed to serialize email token request");

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<ThirdPartyTokenResponse>(r) {
            tx.send(BKResponse::GetTokenEmail(response.sid, client_secret))
                .unwrap();
        } else {
            tx.send(BKResponse::GetTokenEmailError(Error::BackendError))
                .unwrap();
        },
        |err| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                tx.send(BKResponse::GetTokenEmailUsed).unwrap();
            }
            _ => {
                tx.send(BKResponse::GetTokenEmailError(err)).unwrap();
            }
        }
    );

    Ok(())
}

pub fn get_phone_token(
    bk: &Backend,
    identity: String,
    phone: String,
    client_secret: String,
) -> Result<(), Error> {
    let url = bk.url(&format!("account/3pid/msisdn/requestToken"), vec![])?;

    let attrs = PhoneTokenRequest {
        id_server: identity[8..].into(),
        client_secret: client_secret.clone(),
        phone_number: phone,
        country: String::new(),
        send_attempt: 1,
        next_link: None,
    };

    let attrs_json = serde_json::to_value(attrs).expect("Failed to serialize phone token request");

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<ThirdPartyTokenResponse>(r) {
            tx.send(BKResponse::GetTokenPhone(response.sid, client_secret))
                .unwrap();
        } else {
            tx.send(BKResponse::GetTokenPhoneError(Error::BackendError))
                .unwrap();
        },
        |err| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                tx.send(BKResponse::GetTokenPhoneUsed).unwrap();
            }
            _ => {
                tx.send(BKResponse::GetTokenPhoneError(err)).unwrap();
            }
        }
    );

    Ok(())
}

pub fn add_threepid(
    bk: &Backend,
    identity: String,
    client_secret: String,
    sid: String,
) -> Result<(), Error> {
    let url = bk.url(&format!("account/3pid"), vec![])?;
    let attrs = AddThreePIDRequest {
        three_pid_creds: ThreePIDCredentials {
            id_server: identity[8..].into(),
            sid: sid.clone(),
            client_secret,
        },
        bind: true,
    };

    let attrs_json = serde_json::to_value(attrs)
        .expect("Failed to serialize add third party information request");

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |_| {
            tx.send(BKResponse::AddThreePID(sid)).unwrap();
        },
        |err| {
            tx.send(BKResponse::AddThreePIDError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn submit_phone_token(
    bk: &Backend,
    url: &str,
    client_secret: String,
    sid: String,
    token: String,
) -> Result<(), Error> {
    let path = "/_matrix/identity/api/v1/validate/msisdn/submitToken";
    let url = build_url(&Url::parse(url)?, path, &[])?;

    let attrs = SubmitPhoneTokenRequest {
        sid: sid.clone(),
        client_secret: client_secret.clone(),
        token,
    };

    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize phone token submit request");
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<SubmitPhoneTokenResponse>(r) {
            let result = Some(sid).filter(|_| response.success);
            tx.send(BKResponse::SubmitPhoneToken(result, client_secret))
                .unwrap();
        } else {
            tx.send(BKResponse::SubmitPhoneTokenError(Error::BackendError))
                .unwrap();
        },
        |err| {
            tx.send(BKResponse::SubmitPhoneTokenError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn delete_three_pid(bk: &Backend, medium: Medium, address: String) {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let mut url = baseu
        .join("/_matrix/client/r0/account/3pid/delete")
        .expect("Wrong URL in delete_three_pid()");
    url.query_pairs_mut()
        .clear()
        .append_pair("access_token", &tk);
    let attrs = DeleteThreePIDRequest { medium, address };

    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize third party ID delete request");
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |_r: JsonValue| {
            tx.send(BKResponse::DeleteThreePID).unwrap();
        },
        |err| {
            tx.send(BKResponse::DeleteThreePIDError(err)).unwrap();
        }
    );
}

pub fn change_password(
    bk: &Backend,
    user: String,
    old_password: String,
    new_password: String,
) -> Result<(), Error> {
    let url = bk.url(&format!("account/password"), vec![])?;

    let attrs = ChangePasswordRequest {
        new_password,
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password: old_password,
            session: None,
        }),
    };

    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize password change request");
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| {
            info!("{}", r);
            tx.send(BKResponse::ChangePassword).unwrap();
        },
        |err| {
            tx.send(BKResponse::ChangePasswordError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn account_destruction(bk: &Backend, user: String, password: String) -> Result<(), Error> {
    let url = bk.url(&format!("account/deactivate"), vec![])?;

    let attrs = DeactivateAccountRequest {
        auth: Some(AuthenticationData::Password {
            identifier: Identifier::new(UserIdentifier::User { user }),
            password,
            session: None,
        }),
    };

    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize account deactivation request");
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| {
            info!("{}", r);
            tx.send(BKResponse::AccountDestruction).unwrap();
        },
        |err| {
            tx.send(BKResponse::AccountDestructionError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn get_avatar(bk: &Backend) -> Result<(), Error> {
    let baseu = bk.get_base_url();
    let userid = bk.data.lock().unwrap().user_id.clone();

    let tx = bk.tx.clone();
    thread::spawn(move || match get_user_avatar(&baseu, &userid) {
        Ok((_, fname)) => {
            tx.send(BKResponse::Avatar(fname)).unwrap();
        }
        Err(err) => {
            tx.send(BKResponse::AvatarError(err)).unwrap();
        }
    });

    Ok(())
}

pub fn get_user_info_async(
    bk: &mut Backend,
    uid: &str,
    tx: Option<Sender<(String, String)>>,
) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    let u = String::from(uid);

    if let Some(info) = bk.user_info_cache.get(&u) {
        if let Some(tx) = tx.clone() {
            let info = info.clone();
            thread::spawn(move || {
                let i = info.lock().unwrap().clone();
                tx.send(i).unwrap();
            });
        }
        return Ok(());
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

    Ok(())
}

pub fn get_username_async(bk: &Backend, uid: String, tx: Sender<String>) -> Result<(), Error> {
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&uid)), vec![])?;
    get!(
        &url,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<GetDisplayNameResponse>(r) {
            let name = response.displayname.unwrap_or(uid);
            tx.send(name).unwrap();
        } else {
            tx.send(uid.to_string()).unwrap();
        },
        |_| tx.send(uid.to_string()).unwrap()
    );

    Ok(())
}

pub fn get_avatar_async(
    bk: &Backend,
    member: Option<Member>,
    tx: Sender<String>,
) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    if member.is_none() {
        tx.send(String::new()).unwrap();
        return Ok(());
    }

    let m = member.unwrap();

    let uid = m.uid.clone();
    let avatar = m.avatar.clone();

    semaphore(bk.limit_threads.clone(), move || match get_user_avatar_img(
        &baseu,
        &uid,
        &avatar.unwrap_or_default(),
    ) {
        Ok(fname) => {
            tx.send(fname.clone()).unwrap();
        }
        Err(_) => {
            tx.send(String::new()).unwrap();
        }
    });

    Ok(())
}

pub fn set_user_avatar(bk: &Backend, avatar: String) -> Result<(), Error> {
    let baseu = bk.get_base_url();
    let id = bk.data.lock().unwrap().user_id.clone();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = &[("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", params)?;
    let url = bk.url(&format!("profile/{}/avatar_url", encode_uid(&id)), vec![])?;

    let mut file = File::open(&avatar)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let tx = bk.tx.clone();
    thread::spawn(move || {
        match put_media(mediaurl.as_str(), contents) {
            Err(err) => {
                tx.send(BKResponse::SetUserAvatarError(err)).unwrap();
            }
            Ok(js) => {
                let uri = js["content_uri"].as_str().unwrap_or_default();
                let attrs = json!({ "avatar_url": uri });
                put!(
                    &url,
                    &attrs,
                    |_| tx.send(BKResponse::SetUserAvatar(avatar)).unwrap(),
                    |err| tx.send(BKResponse::SetUserAvatarError(err)).unwrap(),
                    0
                );
            }
        };
    });

    Ok(())
}

pub fn search(bk: &Backend, search_term: String) -> Result<(), Error> {
    let url = bk.url(&format!("user_directory/search"), vec![])?;

    let attrs = SearchUserRequest {
        search_term,
        ..Default::default()
    };

    let attrs_json =
        serde_json::to_value(attrs).expect("Failed to serialize user directory search request");
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<SearchUserResponse>(r) {
            let users = response.results.into_iter().map(Into::into).collect();
            tx.send(BKResponse::UserSearch(users)).unwrap();
        } else {
            tx.send(BKResponse::CommandError(Error::BackendError))
                .unwrap();
        },
        |err| {
            tx.send(BKResponse::CommandError(err)).unwrap();
        }
    );

    Ok(())
}
