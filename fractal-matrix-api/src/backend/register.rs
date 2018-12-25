use std::thread;
use url::Url;
use JsonValue;

use error::Error;
use globals;
use util::json_q;

pub use backend::types::{BKResponse, Backend};

impl Backend {
    pub fn guest(&self, server: String) {
        let ctx = self.tx.clone();
        let url = Url::parse(&server)
            .unwrap()
            .join("/_matrix/client/r0/register?kind=guest")
            .unwrap();
        self.data.lock().unwrap().server_url = server;

        let data = self.data.clone();
        let attrs = json!({});
        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                let uid = r["user_id"].as_str().unwrap_or_default().to_string();
                let tk = r["access_token"].as_str().unwrap_or_default().to_string();
                let dev = r["device_id"].as_str().unwrap_or_default().to_string();
                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                ctx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
                ctx.send(BKResponse::Rooms(vec![], None)).unwrap();
            },
            |err| ctx.send(BKResponse::GuestLoginError(err)).unwrap()
        );
    }

    pub fn login(&self, user: String, password: String, server: String) {
        let ctx = self.tx.clone();
        self.data.lock().unwrap().server_url = server;
        let url = self.url("login", vec![]);

        let attrs = build_login_attrs(&user, &password);
        let data = self.data.clone();

        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                let uid = r["user_id"].as_str().unwrap_or_default().to_string();
                let tk = r["access_token"].as_str().unwrap_or_default().to_string();
                let dev = r["device_id"].as_str().unwrap_or_default().to_string();

                if uid.is_empty() || tk.is_empty() {
                    ctx.send(BKResponse::LoginError(Error::BackendError))
                        .unwrap();
                } else {
                    data.lock().unwrap().user_id = uid.clone();
                    data.lock().unwrap().access_token = tk.clone();
                    data.lock().unwrap().since = None;
                    ctx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
                }
            },
            |err| ctx.send(BKResponse::LoginError(err)).unwrap()
        );
    }

    pub fn set_token(&self, token: String, uid: String, server: String) {
        let ctx = self.tx.clone();
        self.data.lock().unwrap().server_url = server;
        self.data.lock().unwrap().access_token = token.clone();
        self.data.lock().unwrap().user_id = uid.clone();
        self.data.lock().unwrap().since = None;
        ctx.send(BKResponse::Token(uid.to_string(), token.to_string(), None))
            .unwrap();
    }

    pub fn logout(&self) {
        let ctx = self.tx.clone();
        let url = self.url("logout", vec![]);
        let attrs = json!({});

        let data = self.data.clone();
        post!(
            &url,
            &attrs,
            |_| {
                data.lock().unwrap().user_id = String::new();
                data.lock().unwrap().access_token = String::new();
                data.lock().unwrap().since = None;
                ctx.send(BKResponse::Logout).unwrap();
            },
            |err| ctx.send(BKResponse::LogoutError(err)).unwrap()
        );
    }

    pub fn register(&self, user: String, password: String, server: String) {
        let ctx = self.tx.clone();
        self.data.lock().unwrap().server_url = server;
        let url = self.url("register", vec![("kind", "user".to_string())]);

        let attrs = json!({
            "auth": {"type": "m.login.password"},
            "username": user,
            "bind_email": false,
            "password": password
        });

        let data = self.data.clone();
        post!(
            &url,
            &attrs,
            |r: JsonValue| {
                let uid = r["user_id"].as_str().unwrap_or_default().to_string();
                let tk = r["access_token"].as_str().unwrap_or_default().to_string();
                let dev = r["device_id"].as_str().unwrap_or_default().to_string();
                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                ctx.send(BKResponse::Token(uid, tk, Some(dev))).unwrap();
            },
            |err| ctx.send(BKResponse::LoginError(err)).unwrap()
        );
    }
}

fn build_login_attrs(user: &str, password: &str) -> JsonValue {
    if globals::EMAIL_RE.is_match(user) {
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
    }
}
