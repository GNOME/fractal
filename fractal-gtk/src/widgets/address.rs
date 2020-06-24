use fractal_api::backend::user;
use fractal_api::r0::AccessToken;
use fractal_api::r0::Medium;
use fractal_api::url::Url;
use glib::signal;
use gtk::prelude::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::thread;

use crate::app::dispatch_error;
use crate::app::App;
use crate::appop::AppOp;
use crate::backend::BKResponse;

#[derive(Debug, Clone)]
pub enum AddressType {
    Email,
    Phone,
}

#[derive(Debug, Clone)]
pub enum AddressAction {
    Delete,
    Add,
}

pub struct Address<'a> {
    op: &'a AppOp,
    entry: gtk::Entry,
    button: gtk::Button,
    action: Option<AddressAction>,
    medium: AddressType,
    address: Option<String>,
    signal_id: Option<signal::SignalHandlerId>,
}

impl<'a> Address<'a> {
    pub fn new(t: AddressType, op: &'a AppOp) -> Address<'a> {
        let entry = gtk::Entry::new();
        let button = gtk::Button::new();
        Address {
            op,
            entry,
            button,
            action: None,
            address: None,
            signal_id: None,
            medium: t,
        }
    }

    pub fn create(&mut self, text: Option<String>) -> gtk::Box {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        b.pack_start(&self.entry, true, true, 0);
        b.pack_end(&self.button, false, false, 0);

        if let Some(text) = text {
            self.address = Some(text.clone());
            self.entry.set_text(&text);
            self.entry.set_editable(false);

            self.action = Some(AddressAction::Delete);
            let label =
                gtk::Image::new_from_icon_name(Some("user-trash-symbolic"), gtk::IconSize::Menu);
            self.button.set_image(Some(&label));
            self.button.show();
        } else {
            let text = match self.medium {
                AddressType::Email => "Add Email",
                AddressType::Phone => "Add Phone",
            };

            self.entry.set_placeholder_text(Some(text));
            self.action = Some(AddressAction::Add);
            let label =
                gtk::Image::new_from_icon_name(Some("list-add-symbolic"), gtk::IconSize::Menu);
            self.button.set_image(Some(&label));
            self.button
                .get_style_context()
                .add_class("suggested-action");
            self.button.hide();
            self.entry.set_editable(true);
        }
        b.get_style_context().add_class("linked");
        self.entry.show();
        self.connect();
        b.show();
        b
    }

    pub fn update(&mut self, text: Option<String>) {
        if let Some(text) = text {
            self.address = Some(text.clone());
            /* Add prefix(+) to phone numbers */
            let text = match self.medium {
                AddressType::Email => text,
                AddressType::Phone => String::from("+") + &text,
            };

            self.entry.set_text(&text);
            self.entry.set_editable(false);

            self.action = Some(AddressAction::Delete);
            let label =
                gtk::Image::new_from_icon_name(Some("user-trash-symbolic"), gtk::IconSize::Menu);
            self.button.set_image(Some(&label));
            self.button
                .get_style_context()
                .remove_class("suggested-action");
            self.button.show();
        } else {
            self.action = Some(AddressAction::Add);
            let label =
                gtk::Image::new_from_icon_name(Some("list-add-symbolic"), gtk::IconSize::Menu);
            self.button.set_image(Some(&label));
            self.button
                .get_style_context()
                .add_class("suggested-action");
            self.button.hide();
            self.entry.set_editable(true);
        }

        self.remove_handler();
        self.connect();
    }

    fn remove_handler(&mut self) {
        let id = self.signal_id.take();
        if let Some(id) = id {
            signal::signal_handler_disconnect(&self.button, id);
        }
    }

    fn connect(&mut self) {
        let login_data = unwrap_or_unit_return!(self.op.login_data.clone());
        let button = self.button.clone();
        let medium = self.medium.clone();
        self.entry.connect_property_text_notify(move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    /* FIXME: use better validation */
                    match medium {
                        AddressType::Email => {
                            button.set_sensitive(text.contains('@') && text.contains('.'));
                        }
                        AddressType::Phone => {}
                    };
                    button.show();
                } else {
                    button.hide();
                }
            }
        });

        let button = self.button.clone();
        self.entry.connect_activate(move |w| {
            if w.get_editable() {
                let _ = button.emit("clicked", &[]);
            }
        });

        let medium = self.medium.clone();
        let action = self.action.clone();
        let entry = self.entry.clone();
        let address = self.address.clone();
        let access_token = login_data.access_token;
        let server_url = login_data.server_url;
        let id_server = login_data.identity_url;
        self.signal_id = Some(self.button.clone().connect_clicked(move |w| {
            if !w.get_sensitive() || !w.is_visible() {
                return;
            }

            let spinner = gtk::Spinner::new();
            spinner.start();
            w.set_image(Some(&spinner));
            w.set_sensitive(false);
            entry.set_editable(false);

            let medium = match medium {
                AddressType::Email => Medium::Email,
                AddressType::Phone => Medium::MsIsdn,
            };

            match action {
                Some(AddressAction::Delete) => {
                    if let Some(address) = address.clone() {
                        delete_address(medium, address, server_url.clone(), access_token.clone());
                    }
                }
                Some(AddressAction::Add) => {
                    if let Some(address) = entry.get_text().map(|gstr| gstr.to_string()) {
                        add_address(
                            medium,
                            id_server.clone(),
                            address,
                            server_url.clone(),
                            access_token.clone(),
                        );
                    }
                }
                _ => {}
            }
        }));
    }
}

fn delete_address(medium: Medium, address: String, server_url: Url, access_token: AccessToken) {
    thread::spawn(move || {
        match user::delete_three_pid(server_url, access_token, medium, address) {
            Ok(_) => {
                APPOP!(get_three_pid);
            }
            Err(err) => {
                dispatch_error(BKResponse::DeleteThreePIDError(err));
            }
        }
    });
}

fn add_address(
    medium: Medium,
    id_server: Url,
    address: String,
    server_url: Url,
    access_token: AccessToken,
) {
    let secret: String = thread_rng().sample_iter(&Alphanumeric).take(36).collect();
    thread::spawn(move || match medium {
        Medium::MsIsdn => {
            match user::get_phone_token(server_url, access_token, id_server, address, secret) {
                Ok((sid, secret)) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_phone, (sid, secret))
                }
                Err(err) => {
                    dispatch_error(BKResponse::GetTokenPhoneError(err));
                }
            }
        }
        Medium::Email => {
            match user::get_email_token(server_url, access_token, id_server, address, secret) {
                Ok((sid, secret)) => {
                    let sid = Some(sid);
                    let secret = Some(secret);
                    APPOP!(get_token_email, (sid, secret));
                }
                Err(err) => {
                    dispatch_error(BKResponse::GetTokenEmailError(err));
                }
            }
        }
    });
}
