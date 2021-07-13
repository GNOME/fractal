use gtk::{gdk, gdk_pixbuf::Pixbuf, gio, glib, glib::clone, prelude::*, subclass::prelude::*};

use log::error;
use matrix_sdk::{
    media::{MediaFormat, MediaRequest, MediaType},
    ruma::identifiers::MxcUri,
};

use crate::utils::do_async;

use crate::session::Session;

mod imp {
    use super::*;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default)]
    pub struct Avatar {
        pub image: RefCell<Option<gdk::Paintable>>,
        pub needed: Cell<bool>,
        pub url: RefCell<Option<MxcUri>>,
        pub display_name: RefCell<Option<String>>,
        pub session: OnceCell<Session>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Avatar {
        const NAME: &'static str = "Avatar";
        type Type = super::Avatar;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Avatar {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "image",
                        "Image",
                        "The user defined image if any",
                        gdk::Paintable::static_type(),
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_object(
                        "needed",
                        "Needed",
                        "Whether the user defined image needs to be loaded",
                        gdk::Paintable::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_string(
                        "url",
                        "Url",
                        "The url of the Avatar",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_string(
                        "display-name",
                        "Display Name",
                        "The display name used for this avatar",
                        None,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "session",
                        "Session",
                        "The session",
                        Session::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "needed" => obj.set_needed(value.get().unwrap()),
                "url" => obj.set_url(value.get::<Option<&str>>().unwrap().map(Into::into)),
                "session" => self.session.set(value.get().unwrap()).unwrap(),
                "display-name" => {
                    let _ = obj.set_display_name(value.get().unwrap());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "image" => obj.image().to_value(),
                "needed" => obj.needed().to_value(),
                "url" => obj.url().map_or_else(
                    || {
                        let none: Option<&str> = None;
                        none.to_value()
                    },
                    |url| url.as_str().to_value(),
                ),
                "display-name" => obj.display_name().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Avatar(ObjectSubclass<imp::Avatar>);
}

/// This an object that holds information about a Users or Rooms `Avatar`
impl Avatar {
    pub fn new(session: &Session, url: Option<MxcUri>) -> Self {
        glib::Object::new(&[
            ("session", session),
            ("url", &url.map(|url| url.to_string())),
        ])
        .expect("Failed to create Avatar")
    }

    fn session(&self) -> &Session {
        let priv_ = imp::Avatar::from_instance(self);
        priv_.session.get().unwrap()
    }

    pub fn image(&self) -> Option<gdk::Paintable> {
        let priv_ = imp::Avatar::from_instance(self);
        priv_.image.borrow().clone()
    }

    fn set_image_data(&self, data: Option<Vec<u8>>) {
        let priv_ = imp::Avatar::from_instance(self);

        let image = if let Some(data) = data {
            let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(&data));
            Pixbuf::from_stream(&stream, gio::NONE_CANCELLABLE)
                .ok()
                .and_then(|pixbuf| Some(gdk::Texture::for_pixbuf(&pixbuf).upcast()))
        } else {
            None
        };
        priv_.image.replace(image);
        self.notify("image");
    }

    fn load(&self) {
        // Don't do anything here if we don't need the avatar
        if !self.needed() {
            return;
        }

        if let Some(url) = self.url() {
            let client = self.session().client().clone();
            let request = MediaRequest {
                media_type: MediaType::Uri(url),
                format: MediaFormat::File,
            };
            do_async(
                glib::PRIORITY_LOW,
                async move { client.get_media_content(&request, true).await },
                clone!(@weak self as obj => move |result| async move {
                    // FIXME: We should retry if the request failed
                    match result {
                        Ok(data) => obj.set_image_data(Some(data)),
                        Err(error) => error!("Couldn’t fetch avatar: {}", error),
                    };
                }),
            );
        }
    }

    pub fn set_display_name(&self, display_name: Option<String>) {
        let priv_ = imp::Avatar::from_instance(self);
        if self.display_name() == display_name {
            return;
        }

        priv_.display_name.replace(display_name);

        self.notify("display-name");
    }

    pub fn display_name(&self) -> Option<String> {
        let priv_ = imp::Avatar::from_instance(self);
        priv_.display_name.borrow().clone()
    }

    pub fn set_needed(&self, needed: bool) {
        let priv_ = imp::Avatar::from_instance(self);
        if self.needed() == needed {
            return;
        }

        priv_.needed.set(needed);

        if needed {
            self.load();
        }

        self.notify("needed");
    }

    pub fn needed(&self) -> bool {
        let priv_ = imp::Avatar::from_instance(self);
        priv_.needed.get()
    }

    pub fn set_url(&self, url: Option<MxcUri>) {
        let priv_ = imp::Avatar::from_instance(self);

        if priv_.url.borrow().as_ref() == url.as_ref() {
            return;
        }

        let has_url = url.is_some();
        priv_.url.replace(url);

        if has_url {
            self.load();
        } else {
            self.set_image_data(None);
        }

        self.notify("url");
    }

    pub fn url(&self) -> Option<MxcUri> {
        let priv_ = imp::Avatar::from_instance(self);
        priv_.url.borrow().to_owned()
    }
}
