use std::thread;
use url::Url;

use crate::error::Error;

use crate::globals;
use crate::r0::account::login::request as login_req;
use crate::r0::account::login::Auth;
use crate::r0::account::login::Body as LoginBody;
use crate::r0::account::login::Response as LoginResponse;
use crate::r0::account::logout::request as logout_req;
use crate::r0::account::logout::Parameters as LogoutParameters;
use crate::r0::account::register::request as register_req;
use crate::r0::account::register::Body as RegisterBody;
use crate::r0::account::register::Parameters as RegisterParameters;
use crate::r0::account::register::RegistrationKind;
use crate::r0::account::register::Response as RegisterResponse;
use crate::r0::account::Identifier;
use crate::r0::account::Medium;
use crate::r0::account::UserIdentifier;
use crate::util::HTTP_CLIENT;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;

pub fn guest(bk: &Backend, server: &str) -> Result<(), Error> {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let base = Url::parse(server)?;
    data.lock().unwrap().server_url = base.clone();

    let params = RegisterParameters {
        kind: RegistrationKind::Guest,
    };
    let body = Default::default();

    thread::spawn(move || {
        let query = register_req(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<RegisterResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let uid = response.user_id;
                let tk = response.access_token.unwrap_or_default();
                let dev = response.device_id;

                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                let _ = tx.send(BKResponse::Token(uid, tk, dev));
                let _ = tx.send(BKResponse::Rooms(vec![], None));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::GuestLoginError(err));
            }
        }
    });

    Ok(())
}

pub fn login(bk: &Backend, user: String, password: String, server: &str) -> Result<(), Error> {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let base = Url::parse(server)?;
    data.lock().unwrap().server_url = base.clone();

    let body = if globals::EMAIL_RE.is_match(&user) {
        LoginBody {
            auth: Auth::Password { password },
            identifier: Identifier::new(UserIdentifier::ThirdParty {
                medium: Medium::Email,
                address: user.clone(),
            }),
            initial_device_display_name: Some(globals::DEVICE_NAME.into()),
            device_id: None,
        }
    } else {
        LoginBody {
            auth: Auth::Password { password },
            identifier: Identifier::new(UserIdentifier::User { user: user.clone() }),
            initial_device_display_name: Some(globals::DEVICE_NAME.into()),
            device_id: None,
        }
    };

    thread::spawn(move || {
        let query = login_req(base, &body)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<LoginResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let uid = response.user_id.unwrap_or(user);
                let tk = response.access_token.unwrap_or_default();
                let dev = response.device_id;

                if uid.is_empty() || tk.is_empty() {
                    let _ = tx.send(BKResponse::LoginError(Error::BackendError));
                } else {
                    data.lock().unwrap().user_id = uid.clone();
                    data.lock().unwrap().access_token = tk.clone();
                    data.lock().unwrap().since = None;
                    let _ = tx.send(BKResponse::Token(uid, tk, dev));
                }
            }
            Err(err) => {
                let _ = tx.send(BKResponse::LoginError(err));
            }
        }
    });

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

pub fn logout(bk: &Backend) {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let base = bk.get_base_url();
    let params = LogoutParameters {
        access_token: data.lock().unwrap().access_token.clone(),
    };

    thread::spawn(move || {
        let query = logout_req(base, &params)
            .and_then(|request| HTTP_CLIENT.execute(request))
            .map_err(Into::into);

        match query {
            Ok(_) => {
                data.lock().unwrap().user_id = Default::default();
                data.lock().unwrap().access_token = Default::default();
                data.lock().unwrap().since = None;
                let _ = tx.send(BKResponse::Logout);
            }
            Err(err) => {
                let _ = tx.send(BKResponse::LogoutError(err));
            }
        }
    });
}

pub fn register(bk: &Backend, user: String, password: String, server: &str) -> Result<(), Error> {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let base = Url::parse(server)?;
    data.lock().unwrap().server_url = base.clone();
    let params = Default::default();
    let body = RegisterBody {
        username: Some(user),
        password: Some(password),
        ..Default::default()
    };

    thread::spawn(move || {
        let query = register_req(base, &params, &body)
            .and_then(|request| HTTP_CLIENT.execute(request)?.json::<RegisterResponse>())
            .map_err(Into::into);

        match query {
            Ok(response) => {
                let uid = response.user_id;
                let tk = response.access_token.unwrap_or_default();
                let dev = response.device_id;

                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = None;
                let _ = tx.send(BKResponse::Token(uid, tk, dev));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::LoginError(err));
            }
        }
    });

    Ok(())
}
