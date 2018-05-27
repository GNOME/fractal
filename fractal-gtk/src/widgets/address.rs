extern crate gtk;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use self::gtk::prelude::*;

use widgets;

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

#[derive(Debug, Clone)]
pub struct Address {
    entry: gtk::Entry,
    button: gtk::Button,
    action: Option<AddressAction>,
    medium: AddressType,
}

impl Address {
    pub fn new(t: AddressType) -> Address {
        let entry = gtk::Entry::new();
        let button = gtk::Button::new();
        Address {
            entry: entry,
            button: button,
            action: None,
            medium: t,
        }
    }

    pub fn create(mut self, text: Option<String>) -> gtk::Box {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        b.pack_start(&self.entry, true, true, 0);
        b.pack_end(&self.button, false, false, 0);
        if let Some(text) = text {
            self.entry.set_text(&text);

            self.action = Some(AddressAction::Delete);
            let label = gtk::Image::new_from_icon_name("user-trash-symbolic", 1);
            self.button.set_image(&label);
            self.button.show();
        }
        else {
            self.action = Some(AddressAction::Add);
            let label = gtk::Image::new_from_icon_name("list-add-symbolic", 1);
            self.button.set_image(&label);
            if let Some(style) = self.button.get_style_context() {
                style.add_class("suggested-action");
            }
            self.button.hide();
        }
        if let Some(style) = b.get_style_context() {
            style.add_class("linked");
        }
        self.entry.show();
        self.connect();
        b.show();
        b
    }

    pub fn update(mut self, text: Option<String>) {
        if let Some(text) = text {
            self.entry.set_text(&text);

            self.action = Some(AddressAction::Delete);
            let label = gtk::Image::new_from_icon_name("user-trash-symbolic", 1);
            self.button.set_image(&label);
            if let Some(style) = self.button.get_style_context() {
                style.remove_class("suggested-action");
            }
            self.button.show();
        }
        else {
            self.action = Some(AddressAction::Add);
            let label = gtk::Image::new_from_icon_name("list-add-symbolic", 1);
            self.button.set_image(&label);
            if let Some(style) = self.button.get_style_context() {
                style.add_class("suggested-action");
            }
            self.button.hide();
        }
    }


    fn connect(self) {
        let button = self.button.clone();
        let medium = self.medium.clone();
        self.entry.connect_property_text_notify(move |w| { 
            if let Some(text) = w.get_text() {
                if text != "" {
                    /* FIXME: use better validation */
                    match medium {
                        AddressType::Email => button.set_sensitive(text.contains("@") && text.contains(".")),
                        AddressType::Phone => {},
                    };
                    button.show();
                }
                else {
                    button.hide();
                }
            }
        });

        let button = self.button.clone();
        self.entry.connect_activate(move |_| { 
            let _ = button.emit("clicked", &[]);
        });

        let medium = self.medium.clone();
        let action = self.action.clone();
        self.button.connect_clicked(move |w| { 
            if w.get_sensitive() && w.is_visible() {
                match medium {
                    AddressType::Email => {
                        if let Some(ref action) = action {
                            match action {
                                AddressAction::Delete => println!("Delete email number"),
                                AddressAction::Add => println!("Add email number"),
                            }
                        }
                    },
                    AddressType::Phone => {
                        if let Some(ref action) = action {
                            match action {
                                AddressAction::Delete => println!("Delete phone number"),
                                AddressAction::Add => println!("Add phone number"),
                            }
                        }
                    },
                };
            }
        });
    }
}
