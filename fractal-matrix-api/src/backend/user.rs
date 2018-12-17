use std::fs;

use backend::types::BKResponse;
use backend::types::Backend;
use error::Error;
use globals;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use url::Url;
use util::encode_uid;
use util::get_user_avatar;
use util::get_user_avatar_img;
use util::json_q;
use util::media_url;
use util::put_media;
use util::semaphore;

use types::Member;
use types::UserInfo;

use serde_json;
use serde_json::Value as JsonValue;

pub fn get_username(bk: &Backend) -> Result<(), Error> {
    let id = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![])?;
    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let name = r["displayname"].as_str().unwrap_or(&id).to_string();
            tx.send(BKResponse::Name(name)).unwrap();
        },
        |err| tx.send(BKResponse::UserNameError(err)).unwrap()
    );

    Ok(())
}

pub fn set_username(bk: &Backend, name: String) -> Result<(), Error> {
    let id = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![])?;

    let attrs = json!({
        "displayname": name,
    });

    let tx = bk.tx.clone();
    put!(
        &url,
        &attrs,
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
        |r: JsonValue| {
            let result = r["threepids"]
                .as_array()
                .iter()
                .flat_map(|arr| arr.iter())
                .map(|pid| UserInfo {
                    address: pid["address"].as_str().unwrap_or_default().to_string(),
                    added_at: pid["added_at"].as_u64().unwrap_or_default(),
                    validated_at: pid["validated_at"].as_u64().unwrap_or_default(),
                    medium: pid["medium"].as_str().unwrap_or_default().to_string(),
                })
                .collect();
            tx.send(BKResponse::GetThreePID(result)).unwrap();
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
    let url = bk.url(&format!("account/3pid/email/requestToken"), vec![])?;

    let attrs = json!({
        "id_server": identity[8..],
        "client_secret": client_secret.clone(),
        "email": email,
        "send_attempt": "1",
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let sid = r["sid"].as_str().unwrap_or_default().to_string();
            tx.send(BKResponse::GetTokenEmail(sid, client_secret))
                .unwrap();
        },
        |err| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                tx.send(BKResponse::GetTokenEmailUsed).unwrap()
            }
            _ => tx.send(BKResponse::GetTokenEmailError(err)).unwrap(),
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

    let attrs = json!({
        "id_server": identity[8..],
        "client_secret": client_secret,
        "phone_number": phone,
        "country": "",
        "send_attempt": "1",
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let sid = r["sid"].as_str().unwrap_or_default().to_string();
            tx.send(BKResponse::GetTokenPhone(sid, client_secret))
                .unwrap();
        },
        |err| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
            {
                tx.send(BKResponse::GetTokenPhoneUsed).unwrap()
            }
            _ => tx.send(BKResponse::GetTokenPhoneError(err)).unwrap(),
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
    let attrs = json!({
        "three_pid_creds": {
            "id_server": identity[8..],
            "sid": sid,
            "client_secret": client_secret.clone()
        },
        "bind": true
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |_: JsonValue| tx.send(BKResponse::AddThreePID(sid)).unwrap(),
        |err| tx.send(BKResponse::AddThreePIDError(err)).unwrap()
    );

    Ok(())
}

pub fn submit_phone_token(
    bk: &Backend,
    url: String,
    client_secret: String,
    sid: String,
    token: String,
) -> Result<(), Error> {
    let params = &[
        ("sid", sid.clone()),
        ("client_secret", client_secret.clone()),
        ("token", token),
    ];
    let baseu = Url::parse(&url)?;
    let path = baseu.join("/_matrix/identity/api/v1/validate/msisdn/submitToken")?;
    let url = Url::parse_with_params(path.as_str(), params)?;

    let tx = bk.tx.clone();
    post!(
        &url,
        |r: JsonValue| {
            let result = if r["success"] == true {
                Some(sid)
            } else {
                None
            };
            tx.send(BKResponse::SubmitPhoneToken(result, client_secret))
                .unwrap();
        },
        |err| {
            tx.send(BKResponse::SubmitPhoneTokenError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn delete_three_pid(bk: &Backend, medium: String, address: String) -> Result<(), Error> {
    let tk = bk.data.lock().unwrap().access_token.clone();
    let baseu = bk.get_base_url()?;
    let url = baseu.join("/_matrix/client/unstable/account/3pid/delete")?;
    let params = &[("access_token", &tk)];
    let url = Url::parse_with_params(url.as_str(), params)?;

    let attrs = json!({
        "medium": medium,
        "address": address,
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |_: JsonValue| tx.send(BKResponse::DeleteThreePID).unwrap(),
        |err| tx.send(BKResponse::DeleteThreePIDError(err)).unwrap()
    );

    Ok(())
}

pub fn change_password(
    bk: &Backend,
    username: String,
    old_password: String,
    new_password: String,
) -> Result<(), Error> {
    let url = bk.url(&format!("account/password"), vec![])?;

    let attrs = json!({
        "new_password": new_password,
        "auth": {
            "type": "m.login.password",
            "user": username,
            "password": old_password,
        }
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
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

pub fn account_destruction(
    bk: &Backend,
    username: String,
    password: String,
    flag: bool,
) -> Result<(), Error> {
    let url = bk.url(&format!("account/deactivate"), vec![])?;

    let attrs = json!({
        "erase": flag,
        "auth": {
            "type": "m.login.password",
            "user": username,
            "password": password,
        }
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
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
    let baseu = bk.get_base_url()?;
    let userid = bk.data.lock().unwrap().user_id.clone();

    let tx = bk.tx.clone();
    thread::spawn(move || match get_user_avatar(&baseu, &userid) {
        Ok((_, fname)) => tx.send(BKResponse::Avatar(fname)).unwrap(),
        Err(err) => tx.send(BKResponse::AvatarError(err)).unwrap(),
    });

    Ok(())
}

pub fn get_user_info_async(
    bk: &mut Backend,
    uid: &str,
    tx: Option<Sender<(String, String)>>,
) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    let u = uid.to_string();

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
        get_user_avatar(&baseu, &u)
            .map(|info| {
                if let Some(tx) = tx.clone() {
                    tx.send(info.clone()).unwrap();
                    let mut i = i0.unwrap();
                    i.0 = info.0;
                    i.1 = info.1;
                }
            })
            .map_err(|_| {
                tx.clone()
                    .map(|tx| tx.send((String::new(), String::new())).unwrap())
            })
            .unwrap_or_default();
    });

    bk.user_info_cache.insert(cache_key, cache_value);

    Ok(())
}

pub fn get_username_async(bk: &Backend, uid: String, tx: Sender<String>) -> Result<(), Error> {
    let url = bk.url(&format!("profile/{}/displayname", encode_uid(&uid)), vec![])?;
    get!(
        &url,
        |r: JsonValue| {
            let name = r["displayname"].as_str().unwrap_or(&uid).to_string();
            tx.send(name).unwrap();
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
    let baseu = bk.get_base_url()?;

    if member.is_none() {
        tx.send(String::new()).unwrap();
        return Ok(());
    }

    let m = member.unwrap();

    let uid = m.uid.clone();
    let avatar = m.avatar.clone();

    semaphore(bk.limit_threads.clone(), move || {
        let fname =
            get_user_avatar_img(&baseu, uid, avatar.unwrap_or_default()).unwrap_or_default();
        tx.send(fname).unwrap()
    });

    Ok(())
}

pub fn set_user_avatar(bk: &Backend, avatar: String) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let id = bk.data.lock().unwrap().user_id.clone();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", params)?;
    let url = bk.url(&format!("profile/{}/avatar_url", encode_uid(&id)), vec![])?;

    let contents = fs::read(&avatar)?;

    let tx = bk.tx.clone();
    thread::spawn(move || match put_media(mediaurl.as_str(), contents) {
        Ok(js) => {
            let uri = js["content_uri"].as_str().unwrap_or_default();
            let attrs = json!({ "avatar_url": uri });
            json_q("put", &url, &attrs, 0)
                .map(|_| tx.send(BKResponse::SetUserAvatar(avatar)).unwrap())
                .map_err(|err| tx.send(BKResponse::SetUserAvatarError(err)).unwrap())
                .unwrap_or_default()
        }
        Err(err) => tx.send(BKResponse::SetUserAvatarError(err)).unwrap(),
    });

    Ok(())
}

pub fn search(bk: &Backend, term: String) -> Result<(), Error> {
    let url = bk.url(&format!("user_directory/search"), vec![])?;

    let attrs = json!({
        "search_term": term,
    });

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |js: JsonValue| {
            let users = js["results"]
                .as_array()
                .iter()
                .flat_map(|arr| arr.iter())
                .map(|member| {
                    let mut member_s: Member = serde_json::from_value(member.clone()).unwrap();
                    member_s.uid = member["user_id"].as_str().unwrap_or_default().to_string();
                    member_s
                })
                .collect();
            tx.send(BKResponse::UserSearch(users)).unwrap();
        },
        |err| tx.send(BKResponse::CommandError(err)).unwrap()
    );

    Ok(())
}
