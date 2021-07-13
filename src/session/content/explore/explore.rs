use crate::{
    session::content::explore::{PublicRoom, PublicRoomList, PublicRoomRow},
    session::Session,
};

use matrix_sdk::ruma::api::client::r0::thirdparty::get_protocols;

use crate::utils::do_async;
use adw::subclass::prelude::*;
use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*, CompositeTemplate};
use log::error;

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-explore.ui")]
    pub struct Explore {
        pub compact: Cell<bool>,
        pub session: RefCell<Option<Session>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub empty_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub network_menu: TemplateChild<gtk::ComboBoxText>,
        #[template_child]
        pub listview: TemplateChild<gtk::ListView>,
        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        pub public_room_list: RefCell<Option<PublicRoomList>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Explore {
        const NAME: &'static str = "ContentExplore";
        type Type = super::Explore;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            PublicRoom::static_type();
            PublicRoomList::static_type();
            PublicRoomRow::static_type();
            Self::bind_template(klass);
            klass.set_accessible_role(gtk::AccessibleRole::Group);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Explore {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boolean(
                        "compact",
                        "Compact",
                        "Whether a compact view is used",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "session",
                        "Session",
                        "The session",
                        Session::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "compact" => self.compact.set(value.get().unwrap()),
                "session" => obj.set_session(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            let adj = self.scrolled_window.vadjustment().unwrap();

            adj.connect_value_changed(clone!(@weak obj => move |adj| {
                if adj.upper() - adj.value() < adj.page_size() * 2.0 {
                    let priv_ = imp::Explore::from_instance(&obj);
                    if let Some(public_room_list) = &*priv_.public_room_list.borrow() {
                        public_room_list.load_public_rooms(false);
                    }
                }
            }));

            self.search_entry
                .connect_search_changed(clone!(@weak obj => move |_| {
                    let priv_ = imp::Explore::from_instance(&obj);
                    if let Some(public_room_list) = &*priv_.public_room_list.borrow() {
                        let text = priv_.search_entry.text().as_str().to_string();
                        let network = priv_.network_menu.active_id().map(|id| id.as_str().to_owned());
                        public_room_list.search(Some(text), None, network);
                    };
                }));
        }
    }

    impl WidgetImpl for Explore {}
    impl BinImpl for Explore {}
}

glib::wrapper! {
    pub struct Explore(ObjectSubclass<imp::Explore>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Explore {
    pub fn new(session: &Session) -> Self {
        glib::Object::new(&[("session", session)]).expect("Failed to create Explore")
    }

    pub fn session(&self) -> Option<Session> {
        let priv_ = imp::Explore::from_instance(self);
        priv_.session.borrow().to_owned()
    }

    pub fn init(&self) {
        let priv_ = imp::Explore::from_instance(self);
        self.load_protocols();
        if let Some(public_room_list) = &*priv_.public_room_list.borrow() {
            public_room_list.load_public_rooms(true);
        }
    }

    pub fn set_session(&self, session: Option<Session>) {
        let priv_ = imp::Explore::from_instance(self);

        if session == self.session() {
            return;
        }

        if let Some(ref session) = session {
            let public_room_list = PublicRoomList::new(session);
            priv_
                .listview
                .set_model(Some(&gtk::NoSelection::new(Some(&public_room_list))));

            public_room_list.connect_notify_local(
                Some("loading"),
                clone!(@weak self as obj => move |_, _| {
                    obj.set_visible_child();
                }),
            );

            public_room_list.connect_notify_local(
                Some("empty"),
                clone!(@weak self as obj => move |_, _| {
                    obj.set_visible_child();
                }),
            );

            priv_.public_room_list.replace(Some(public_room_list));
        }

        priv_.session.replace(session);
        self.notify("session");
    }

    fn set_visible_child(&self) {
        let priv_ = imp::Explore::from_instance(self);
        if let Some(public_room_list) = &*priv_.public_room_list.borrow() {
            if public_room_list.loading() {
                priv_.stack.set_visible_child(&*priv_.spinner);
            } else if public_room_list.empty() {
                priv_.stack.set_visible_child(&*priv_.empty_label);
            } else {
                priv_.stack.set_visible_child(&*priv_.scrolled_window);
            }
        }
    }

    fn set_protocols(&self, protocols: get_protocols::Response) {
        let priv_ = imp::Explore::from_instance(self);

        for protocol in protocols
            .protocols
            .into_iter()
            .flat_map(|(_, protocol)| protocol.instances)
        {
            priv_
                .network_menu
                .append(Some(&protocol.instance_id), &protocol.desc);
        }
    }

    fn load_protocols(&self) {
        let priv_ = imp::Explore::from_instance(self);
        let client = self.session().unwrap().client().clone();

        priv_.network_menu.remove_all();
        priv_.network_menu.append(Some("matrix"), "Matrix");
        priv_.network_menu.append(Some("all"), "All rooms");
        priv_.network_menu.set_active(Some(0));

        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move { client.send(get_protocols::Request::new(), None).await },
            clone!(@weak self as obj => move |result| async move {
                match result {
                 Ok(response) => obj.set_protocols(response),
                 Err(error) => error!("Error loading supported protocols: {}", error),
                }
            }),
        );
    }
}
