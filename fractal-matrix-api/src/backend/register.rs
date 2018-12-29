use regex::Regex;
use serde_json::Value as JsonValue;

use std::thread;
use url::Url;

use error::Error;
use globals;
use util::json_q;

use backend::types::BKResponse;
use backend::types::Backend;

pub fn guest(bk: &Backend, server: &str) -> Result<(), Error> {
    let baseu = Url::parse(server)?;
    let url = baseu
        .join("/_matrix/client/r0/register?kind=guest")
        .unwrap();
    bk.data.lock().unwrap().server_url = baseu;

    let data = bk.data.clone();
    let tx = bk.tx.clone();
    let attrs = json!({});
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let uid = String::from(r["user_id"].as_str().unwrap_or(""));
            let tk = String::from(r["access_token"].as_str().unwrap_or(""));
            let dev = String::from(r["device_id"].as_str().unwrap_or(""));
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

fn build_login_attrs(user: &str, password: &str) -> Result<JsonValue, Error> {
    let emailre = Regex::new(
        r"^([0-9a-zA-Z]([-\.\w]*[0-9a-zA-Z])+@([0-9a-zA-Z][-\w]*[0-9a-zA-Z]\.)+[a-zA-Z]{2,9})$",
    )?;

    // Email
    let attrs = if emailre.is_match(&user) {
        json!({
            "type": "m.login.password",
            "password": password,
            "initial_device_display_name": "Fractal",
            "medium": "email",
            "address": user,
            "identifier": {
                "type": "m.id.thirdparty",
                "medium": "email",
                "address": user,
            }
        })
    } else {
        json!({
            "type": "m.login.password",
            "initial_device_display_name": "Fractal",
            "user": user,
            "password": password
        })
    };

    Ok(attrs)
}

pub fn login(bk: &Backend, user: &str, password: &str, server: &str) -> Result<(), Error> {
    bk.data.lock().unwrap().server_url = Url::parse(server)?;
    let url = bk.url("login", &[])?;

    let attrs = build_login_attrs(user, password)?;
    let data = bk.data.clone();

    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        |r: JsonValue| {
            let uid = String::from(r["user_id"].as_str().unwrap_or(""));
            let tk = String::from(r["access_token"].as_str().unwrap_or(""));
            let dev = String::from(r["device_id"].as_str().unwrap_or(""));

            if uid.is_empty() || tk.is_empty() {
                tx.send(BKResponse::LoginError(Error::BackendError))
                    .unwrap();
            } else {
                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                tx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
            }
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
    let url = bk.url("logout", &[])?;
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
    let url = bk.url("register", &vec![("kind", String::from("user"))])?;

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
            let uid = String::from(r["user_id"].as_str().unwrap_or(""));
            let tk = String::from(r["access_token"].as_str().unwrap_or(""));
            let dev = String::from(r["device_id"].as_str().unwrap_or(""));

            data.lock().unwrap().user_id = uid.clone();
            data.lock().unwrap().access_token = tk.clone();
            data.lock().unwrap().since = None;
            tx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
        },
        |err| tx.send(BKResponse::LoginError(err)).unwrap()
    );

    Ok(())
}
