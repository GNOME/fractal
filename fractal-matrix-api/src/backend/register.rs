use serde_json::json;
use serde_json::Value as JsonValue;

use std::thread;
use url::Url;

use crate::error::Error;
use crate::globals;
use crate::util::json_q;

use crate::types::LoginRequest;
use crate::types::LoginResponse;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;

pub fn guest(bk: &Backend, server: &str) -> Result<(), Error> {
    let baseu = Url::parse(server)?;
    let url = baseu
        .join("/_matrix/client/r0/register?kind=guest")
        .expect("Wrong URL in guest()");
    bk.data.lock().unwrap().server_url = baseu;

    let data = bk.data.clone();
    let tx = bk.tx.clone();
    let attrs = json!({});
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let uid = String::from(r["user_id"].as_str().unwrap_or_default());
            let tk = String::from(r["access_token"].as_str().unwrap_or_default());
            let dev = String::from(r["device_id"].as_str().unwrap_or_default());
            data.lock().unwrap().user_id = uid.clone();
            data.lock().unwrap().access_token = tk.clone();
            data.lock().unwrap().since = None;
            tx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
            tx.send(BKResponse::Rooms(vec![], None)).unwrap();
        },
        |err| tx.send(BKResponse::GuestLoginError(err)).unwrap()
    );

    Ok(())
}

pub fn login(bk: &Backend, user: String, password: String, server: &str) -> Result<(), Error> {
    bk.data.lock().unwrap().server_url = Url::parse(server)?;
    let url = bk.url("login", vec![])?;

    let attrs = LoginRequest::new(user.clone(), password);
    let attrs_json = serde_json::to_value(attrs).expect("Failed to serialize login request");
    let data = bk.data.clone();

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs_json,
        |r: JsonValue| if let Ok(response) = serde_json::from_value::<LoginResponse>(r) {
            let uid = response.user_id.unwrap_or(user);
            let tk = response.access_token.unwrap_or_default();
            let dev = response.device_id;

            if uid.is_empty() || tk.is_empty() {
                tx.send(BKResponse::LoginError(Error::BackendError))
                    .unwrap();
            } else {
                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                tx.send(BKResponse::Token(uid, tk, dev)).unwrap();
            }
        } else {
            tx.send(BKResponse::LoginError(Error::BackendError))
                .unwrap();
        },
        |err| tx.send(BKResponse::LoginError(err)).unwrap()
    );

    Ok(())
}

pub fn set_token(bk: &Backend, token: String, uid: String, server: &str) -> Result<(), Error> {
    bk.data.lock().unwrap().server_url = Url::parse(server)?;
    bk.data.lock().unwrap().access_token = token.clone();
    bk.data.lock().unwrap().user_id = uid.clone();
    bk.data.lock().unwrap().since = None;
    bk.tx.send(BKResponse::Token(uid, token, None)).unwrap();

    Ok(())
}

pub fn logout(bk: &Backend) -> Result<(), Error> {
    let url = bk.url("logout", vec![])?;
    let attrs = json!({});

    let data = bk.data.clone();
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |_| {
            data.lock().unwrap().user_id = String::new();
            data.lock().unwrap().access_token = String::new();
            data.lock().unwrap().since = None;
            tx.send(BKResponse::Logout).unwrap();
        },
        |err| tx.send(BKResponse::LogoutError(err)).unwrap()
    );
    Ok(())
}

pub fn register(bk: &Backend, user: &str, password: &str, server: &str) -> Result<(), Error> {
    bk.data.lock().unwrap().server_url = Url::parse(server)?;
    let url = bk.url("register", vec![("kind", String::from("user"))])?;

    let attrs = json!({
        "auth": {"type": "m.login.password"},
        "username": user,
        "bind_email": false,
        "password": password
    });

    let data = bk.data.clone();
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let uid = String::from(r["user_id"].as_str().unwrap_or_default());
            let tk = String::from(r["access_token"].as_str().unwrap_or_default());
            let dev = String::from(r["device_id"].as_str().unwrap_or_default());

            data.lock().unwrap().user_id = uid.clone();
            data.lock().unwrap().access_token = tk.clone();
            data.lock().unwrap().since = None;
            tx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
        },
        |err| tx.send(BKResponse::LoginError(err)).unwrap()
    );

    Ok(())
}
