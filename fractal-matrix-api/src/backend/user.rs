use JsonValue;

use std::{
    fs,
    sync::{mpsc::Sender, Arc, Mutex},
    thread,
};

pub use backend::types::{BKResponse, Backend};
use error::Error;
use globals;
use url::Url;
use util::{
    build_url, encode_uid, get_user_avatar, get_user_avatar_img, json_q, media_url, put_media,
    semaphore,
};

use types::{Member, UserInfo};

use serde_json;

impl Backend {
    pub fn get_username(&self) {
        let ctx = self.tx.clone();
        let id = self.data.lock().unwrap().user_id.clone();
        let url = self.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![]);
        get!(
            &url,
            |r: JsonValue| {
                let name = r["displayname"].as_str().unwrap_or(&id).to_string();
                ctx.send(BKResponse::Name(name)).unwrap();
            },
            |err| ctx.send(BKResponse::UserNameError(err)).unwrap()
        );
    }

    pub fn set_username(&self, name: String) {
        let ctx = self.tx.clone();
        let id = self.data.lock().unwrap().user_id.clone();
        let url = self.url(&format!("profile/{}/displayname", encode_uid(&id)), vec![]);

        let attrs = json!({
            "displayname": name,
        });

        put!(
            &url,
            &attrs,
            |_| ctx.send(BKResponse::SetUserName(name)).unwrap(),
            |err| ctx.send(BKResponse::SetUserNameError(err)).unwrap()
        );
    }

    pub fn get_threepid(&self) {
        let ctx = self.tx.clone();
        let url = self.url("account/3pid", vec![]);
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
                ctx.send(BKResponse::GetThreePID(result)).unwrap();
            },
            |err| ctx.send(BKResponse::GetThreePIDError(err)).unwrap()
        );
    }

    pub fn get_email_token(&self, identity: String, email: String, client_secret: String) {
        let ctx = self.tx.clone();
        let url = self.url("account/3pid/email/requestToken", vec![]);

        let attrs = json!({
            "id_server": identity[8..],
            "client_secret": client_secret,
            "email": email,
            "send_attempt": "1",
        });

        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                let sid = r["sid"].as_str().unwrap_or_default().to_string();
                ctx.send(BKResponse::GetTokenEmail(sid, client_secret))
                    .unwrap();
            },
            |err| match err {
                Error::MatrixError(ref js)
                    if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
                {
                    ctx.send(BKResponse::GetTokenEmailUsed).unwrap()
                }
                _ => ctx.send(BKResponse::GetTokenEmailError(err)).unwrap(),
            }
        );
    }

    pub fn get_phone_token(&self, identity: String, phone: String, client_secret: String) {
        let ctx = self.tx.clone();
        let url = self.url("account/3pid/msisdn/requestToken", vec![]);

        let attrs = json!({
            "id_server": identity[8..],
            "client_secret": client_secret,
            "phone_number": phone,
            "country": "",
            "send_attempt": "1",
        });

        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                let sid = r["sid"].as_str().unwrap_or_default().to_string();
                ctx.send(BKResponse::GetTokenPhone(sid, client_secret))
                    .unwrap();
            },
            |err| match err {
                Error::MatrixError(ref js)
                    if js["errcode"].as_str().unwrap_or_default() == "M_THREEPID_IN_USE" =>
                {
                    ctx.send(BKResponse::GetTokenPhoneUsed).unwrap()
                }
                _ => ctx.send(BKResponse::GetTokenPhoneError(err)).unwrap(),
            }
        );
    }

    pub fn add_threepid(&self, identity: String, client_secret: String, sid: String) {
        let ctx = self.tx.clone();
        let url = self.url("account/3pid", vec![]);
        let attrs = json!({
            "three_pid_creds": {
                "id_server": identity[8..],
                "sid": sid,
                "client_secret": client_secret.clone()
            },
            "bind": true
        });

        post!(
            &url,
            &attrs,
            |_| ctx.send(BKResponse::AddThreePID(sid)).unwrap(),
            |err| ctx.send(BKResponse::AddThreePIDError(err)).unwrap()
        );
    }

    pub fn submit_phone_token(
        &self,
        identity: String,
        client_secret: String,
        sid: String,
        token: String,
    ) {
        let tx = self.tx.clone();
        let r = submit_phone_token(self, identity, client_secret, sid, token);
        bkerror!(r, tx, BKResponse::SubmitPhoneTokenError);
    }

    pub fn delete_threepid(&self, medium: String, address: String) {
        let ctx = self.tx.clone();
        let tk = self.data.lock().unwrap().access_token.clone();
        let baseu = self.get_base_url();
        let url = baseu
            .join("/_matrix/client/unstable/account/3pid/delete")
            .unwrap();
        let params = &[("access_token", &tk)];
        let url = Url::parse_with_params(url.as_str(), params).unwrap();

        let attrs = json!({
            "medium": medium,
            "address": address,
        });

        post!(
            &url,
            &attrs,
            |_| ctx.send(BKResponse::DeleteThreePID).unwrap(),
            |err| ctx.send(BKResponse::DeleteThreePIDError(err)).unwrap()
        );
    }

    pub fn change_password(&self, username: String, old_password: String, new_password: String) {
        let ctx = self.tx.clone();
        let url = self.url("account/password", vec![]);

        let attrs = json!({
            "new_password": new_password,
            "auth": {
                "type": "m.login.password",
                "user": username,
                "password": old_password,
            }
        });

        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                info!("{}", r);
                ctx.send(BKResponse::ChangePassword).unwrap();
            },
            |err| ctx.send(BKResponse::ChangePasswordError(err)).unwrap()
        );
    }

    pub fn account_destruction(&self, username: String, password: String, flag: bool) {
        let ctx = self.tx.clone();
        let url = self.url("account/deactivate", vec![]);

        let attrs = json!({
            "erase": flag,
            "auth": {
                "type": "m.login.password",
                "user": username,
                "password": password,
            }
        });

        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                info!("{}", r);
                ctx.send(BKResponse::AccountDestruction).unwrap();
            },
            |err| ctx.send(BKResponse::AccountDestructionError(err)).unwrap()
        );
    }

    pub fn get_avatar(&self) {
        let ctx = self.tx.clone();
        let baseu = self.get_base_url();
        let userid = self.data.lock().unwrap().user_id.clone();

        thread::spawn(move || match get_user_avatar(&baseu, &userid) {
            Ok((_, fname)) => ctx.send(BKResponse::Avatar(fname)).unwrap(),
            Err(err) => ctx.send(BKResponse::AvatarError(err)).unwrap(),
        });
    }

    pub fn get_user_info_async(
        &mut self,
        sender_uid: String,
        ctx: Option<Sender<(String, String)>>,
    ) {
        let baseu = self.get_base_url();

        if let Some(info) = self.user_info_cache.get(&sender_uid) {
            if let Some(ctx) = ctx.clone() {
                let info = info.clone();
                thread::spawn(move || {
                    let i = info.lock().unwrap().clone();
                    ctx.send(i).unwrap();
                });
            }
            return;
        }

        let info = Arc::new(Mutex::new((String::new(), String::new())));
        let cache_key = sender_uid.clone();
        let cache_value = info.clone();

        semaphore(self.limit_threads.clone(), move || {
            let i0 = info.lock();
            get_user_avatar(&baseu, &sender_uid)
                .map(|info| {
                    if let Some(ctx) = ctx.clone() {
                        ctx.send(info.clone()).unwrap();
                        let mut i = i0.unwrap();
                        i.0 = info.0;
                        i.1 = info.1;
                    }
                })
                .map_err(|_| {
                    ctx.clone()
                        .map(|tx| tx.send((String::new(), String::new())).unwrap())
                })
                .unwrap_or_default();
        });

        self.user_info_cache.insert(cache_key, cache_value);
    }

    pub fn get_username_async(&self, sender_uid: String, ctx: Sender<String>) {
        let url = self.url(
            &format!("profile/{}/displayname", encode_uid(&sender_uid)),
            vec![],
        );
        get!(
            &url,
            |r: JsonValue| {
                let name = r["displayname"].as_str().unwrap_or(&sender_uid).to_string();
                ctx.send(name).unwrap();
            },
            |_| ctx.send(sender_uid).unwrap()
        );
    }

    pub fn get_avatar_async(&self, member: Option<Member>, ctx: Sender<String>) {
        let baseu = self.get_base_url();

        if member.is_none() {
            ctx.send(String::new()).unwrap();
            return;
        }

        let m = member.unwrap();

        let uid = m.uid.clone();
        let avatar = m.avatar.clone().unwrap_or_default();

        semaphore(self.limit_threads.clone(), move || {
            let fname = get_user_avatar_img(&baseu, &uid, &avatar).unwrap_or_default();
            ctx.send(fname).unwrap()
        });
    }

    pub fn set_user_avatar(&self, file: String) {
        let tx = self.tx.clone();
        let r = set_user_avatar(self, file);
        bkerror!(r, tx, BKResponse::SetUserAvatarError);
    }

    pub fn user_search(&self, term: String) {
        let ctx = self.tx.clone();
        let url = self.url("user_directory/search", vec![]);

        let attrs = json!({
            "search_term": term,
        });

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
                ctx.send(BKResponse::UserSearch(users)).unwrap();
            },
            |err| ctx.send(BKResponse::CommandError(err)).unwrap()
        );
    }
}

fn submit_phone_token(
    bk: &Backend,
    url: String,
    client_secret: String,
    sid: String,
    token: String,
) -> Result<(), Error> {
    let ctx = bk.tx.clone();
    let params = &[
        ("sid", sid.clone()),
        ("client_secret", client_secret.clone()),
        ("token", token),
    ];
    let path = "/_matrix/identity/api/v1/validate/msisdn/submitToken";
    let url = build_url(&Url::parse(&url)?, path, params)?;

    post!(
        &url,
        |r: JsonValue| {
            let result = if r["success"] == true {
                Some(sid)
            } else {
                None
            };
            ctx.send(BKResponse::SubmitPhoneToken(result, client_secret))
                .unwrap();
        },
        |err| ctx.send(BKResponse::SubmitPhoneTokenError(err)).unwrap()
    );

    Ok(())
}

fn set_user_avatar(bk: &Backend, avatar: String) -> Result<(), Error> {
    let ctx = bk.tx.clone();
    let baseu = bk.get_base_url();
    let id = bk.data.lock().unwrap().user_id.clone();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", &params)?;
    let url = bk.url(&format!("profile/{}/avatar_url", encode_uid(&id)), vec![]);

    let contents = fs::read(&avatar)?;

    thread::spawn(move || match put_media(mediaurl.as_str(), contents) {
        Ok(js) => {
            let uri = js["content_uri"].as_str().unwrap_or_default();
            let attrs = json!({ "avatar_url": uri });
            put!(
                &url,
                &attrs,
                |_| ctx.send(BKResponse::SetUserAvatar(avatar)).unwrap(),
                |err| ctx.send(BKResponse::SetUserAvatarError(err)).unwrap(),
                0
            );
        }
        Err(err) => ctx.send(BKResponse::SetUserAvatarError(err)).unwrap(),
    });

    Ok(())
}
