use fractal_api::r0::account::Medium;
use glib::signal;
use gtk;
use gtk::prelude::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::sync::mpsc::Sender;

use crate::appop::AppOp;
use crate::backend::BKCommand;

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
            op: op,
            entry: entry,
            button: button,
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
            let label = gtk::Image::new_from_icon_name("user-trash-symbolic", gtk::IconSize::Menu);
            self.button.set_image(&label);
            self.button.show();
        } else {
            let text = match self.medium {
                AddressType::Email => "Add Email",
                AddressType::Phone => "Add Phone",
            };

            self.entry.set_placeholder_text(text);
            self.action = Some(AddressAction::Add);
            let label = gtk::Image::new_from_icon_name("list-add-symbolic", gtk::IconSize::Menu);
            self.button.set_image(&label);
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
            let label = gtk::Image::new_from_icon_name("user-trash-symbolic", gtk::IconSize::Menu);
            self.button.set_image(&label);
            self.button
                .get_style_context()
                .remove_class("suggested-action");
            self.button.show();
        } else {
            self.action = Some(AddressAction::Add);
            let label = gtk::Image::new_from_icon_name("list-add-symbolic", gtk::IconSize::Menu);
            self.button.set_image(&label);
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
        let button = self.button.clone();
        let medium = self.medium.clone();
        self.entry.connect_property_text_notify(move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    /* FIXME: use better validation */
                    match medium {
                        AddressType::Email => {
                            button.set_sensitive(text.contains("@") && text.contains("."));
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
        let id_server = self.op.identity_url.clone();
        let backend = self.op.backend.clone();
        self.signal_id = Some(self.button.clone().connect_clicked(move |w| {
            if !w.get_sensitive() || !w.is_visible() {
                return;
            }

            let spinner = gtk::Spinner::new();
            spinner.start();
            w.set_image(&spinner);
            w.set_sensitive(false);
            entry.set_editable(false);

            let medium = match medium {
                AddressType::Email => Medium::Email,
                AddressType::Phone => Medium::MsIsdn,
            };

            match action {
                Some(AddressAction::Delete) => {
                    delete_address(&backend, medium, address.clone());
                }
                Some(AddressAction::Add) => {
                    add_address(
                        &backend,
                        medium,
                        id_server.clone(),
                        entry.get_text().map_or(None, |gstr| Some(gstr.to_string())),
                    );
                }
                _ => {}
            }
        }));
    }
}

fn delete_address(
    backend: &Sender<BKCommand>,
    medium: Medium,
    address: Option<String>,
) -> Option<String> {
    backend
        .send(BKCommand::DeleteThreePID(medium, address?))
        .unwrap();
    None
}

fn add_address(
    backend: &Sender<BKCommand>,
    medium: Medium,
    id_server: String,
    address: Option<String>,
) -> Option<String> {
    let secret: String = thread_rng().sample_iter(&Alphanumeric).take(36).collect();
    match medium {
        Medium::MsIsdn => {
            backend
                .send(BKCommand::GetTokenPhone(id_server, address?, secret))
                .unwrap();
        }
        Medium::Email => {
            backend
                .send(BKCommand::GetTokenEmail(id_server, address?, secret))
                .unwrap();
        }
    }
    None
}
