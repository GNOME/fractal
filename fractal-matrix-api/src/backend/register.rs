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
use crate::r0::account::UserIdentifier;
use crate::r0::server::domain_info::request as domain_info;
use crate::r0::server::domain_info::Response as DomainInfoResponse;
use crate::r0::AccessToken;
use crate::r0::Medium;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;

pub fn guest(bk: &Backend, base: Url) {
    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let params = RegisterParameters {
        kind: RegistrationKind::Guest,
    };
    let body = Default::default();

    thread::spawn(move || {
        let query = register_req(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<RegisterResponse>()
                    .map_err(Into::into)
            });

        match query {
            Ok(response) => {
                let uid = response.user_id.to_string();
                let tk = response.access_token;
                let dev = response.device_id;

                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().since = None;
                tx.send(BKResponse::Token(uid, tk, dev))  // TODO: Use UserId and DeviceId
                    .expect_log("Connection closed");
                tx.send(BKResponse::Rooms(vec![], None))
                    .expect_log("Connection closed");
            }
            Err(err) => {
                tx.send(BKResponse::GuestLoginError(err))
                    .expect_log("Connection closed");
            }
        }
    });
}

pub fn login(bk: &Backend, user: String, password: String, base: Url) {
    let tx = bk.tx.clone();
    let data = bk.data.clone();

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
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<LoginResponse>()
                    .map_err(Into::into)
            });

        match query {
            Ok(response) => {
                let uid = response
                    .user_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(user);
                let tk = response.access_token;
                let dev = response.device_id;

                if uid.is_empty() || tk.is_none() {
                    tx.send(BKResponse::LoginError(Error::BackendError))
                        .expect_log("Connection closed");
                } else {
                    data.lock().unwrap().user_id = uid.clone();
                    data.lock().unwrap().since = None;
                    tx.send(BKResponse::Token(uid, tk, dev))  // TODO: Use UserId and DeviceId
                        .expect_log("Connection closed");
                }
            }
            Err(err) => {
                tx.send(BKResponse::LoginError(err))
                    .expect_log("Connection closed");
            }
        }
    });
}

pub fn set_token(bk: &Backend, token: AccessToken, uid: String) {
    bk.data.lock().unwrap().user_id = uid.clone();
    bk.data.lock().unwrap().since = None;
    bk.tx
        .send(BKResponse::Token(uid, Some(token), None))
        .expect_log("Connection closed");
}

pub fn logout(bk: &Backend, server: Url, access_token: AccessToken) {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let params = LogoutParameters { access_token };

    thread::spawn(move || {
        let query = logout_req(server, &params)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        if query.is_ok() {
            data.lock().unwrap().user_id = Default::default();
            data.lock().unwrap().since = None;
        }

        tx.send(BKResponse::Logout(query))
            .expect_log("Connection closed");
    });
}

pub fn register(bk: &Backend, user: String, password: String, base: Url) {
    let data = bk.data.clone();
    let tx = bk.tx.clone();

    let params = Default::default();
    let body = RegisterBody {
        username: Some(user),
        password: Some(password),
        ..Default::default()
    };

    thread::spawn(move || {
        let query = register_req(base, &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<RegisterResponse>()
                    .map_err(Into::into)
            });

        match query {
            Ok(response) => {
                let uid = response.user_id.to_string();
                let tk = response.access_token;
                let dev = response.device_id;

                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().since = None;
                tx.send(BKResponse::Token(uid, tk, dev))  // TODO: Use UserId
                    .expect_log("Connection closed");
            }
            Err(err) => {
                tx.send(BKResponse::LoginError(err))
                    .expect_log("Connection closed");
            }
        }
    });
}

pub fn get_well_known(domain: Url) -> Result<DomainInfoResponse, Error> {
    domain_info(domain).map_err(Into::into).and_then(|request| {
        HTTP_CLIENT
            .get_client()?
            .execute(request)?
            .json::<DomainInfoResponse>()
            .map_err(Into::into)
    })
}
