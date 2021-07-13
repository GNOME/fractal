mod avatar;
mod content;
mod event_source_dialog;
mod room;
mod room_list;
mod sidebar;
mod user;

pub use self::avatar::Avatar;
use self::content::Content;
pub use self::room::Room;
use self::room_list::RoomList;
use self::sidebar::Sidebar;
pub use self::user::User;

use crate::components::InAppNotification;
use crate::secret;
use crate::secret::StoredSession;
use crate::utils::do_async;
use crate::Error;
use crate::RUNTIME;

use crate::session::content::ContentType;
use adw;
use adw::subclass::prelude::BinImpl;
use gtk::subclass::prelude::*;
use gtk::{self, prelude::*};
use gtk::{gio, glib, glib::clone, glib::SyncSender, CompositeTemplate};
use gtk_macros::send;
use log::error;
use matrix_sdk::ruma::{
    api::client::r0::filter::{FilterDefinition, LazyLoadOptions, RoomEventFilter, RoomFilter},
    assign,
};
use matrix_sdk::{
    deserialized_responses::SyncResponse, uuid::Uuid, Client, ClientConfig, RequestConfig,
    SyncSettings,
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::fs;
use std::time::Duration;
use url::Url;

mod imp {
    use super::*;
    use glib::subclass::{InitializingObject, Signal};
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/session.ui")]
    pub struct Session {
        #[template_child]
        pub error_list: TemplateChild<gio::ListStore>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub content: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub sidebar: TemplateChild<Sidebar>,
        /// Contains the error if something went wrong
        pub error: RefCell<Option<matrix_sdk::Error>>,
        pub client: OnceCell<Client>,
        pub room_list: OnceCell<RoomList>,
        pub user: OnceCell<User>,
        pub selected_room: RefCell<Option<Room>>,
        pub selected_content_type: Cell<ContentType>,
        pub is_ready: OnceCell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Session {
        const NAME: &'static str = "Session";
        type Type = super::Session;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            Sidebar::static_type();
            Content::static_type();
            Error::static_type();
            InAppNotification::static_type();
            obj.init_template();
        }
    }

    impl ObjectImpl for Session {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "room-list",
                        "Room List",
                        "The list of rooms",
                        RoomList::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_object(
                        "selected-room",
                        "Selected Room",
                        "The selected room in this session",
                        Room::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_enum(
                        "selected-content-type",
                        "Selected Content Type",
                        "The current content type selected",
                        ContentType::static_type(),
                        ContentType::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_object(
                        "user",
                        "User",
                        "The user of this session",
                        User::static_type(),
                        glib::ParamFlags::READABLE,
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
                "selected-room" => {
                    let selected_room = value.get().unwrap();
                    obj.set_selected_room(selected_room);
                }
                "selected-content-type" => obj.set_selected_content_type(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "room-list" => obj.room_list().to_value(),
                "selected-room" => obj.selected_room().to_value(),
                "user" => obj.user().to_value(),
                "selected-content-type" => obj.selected_content_type().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("prepared", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for Session {}
    impl BinImpl for Session {}
}

glib::wrapper! {
    pub struct Session(ObjectSubclass<imp::Session>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Session {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Session")
    }

    pub fn selected_content_type(&self) -> ContentType {
        let priv_ = imp::Session::from_instance(self);
        priv_.selected_content_type.get()
    }

    pub fn set_selected_content_type(&self, selected_type: ContentType) {
        let priv_ = imp::Session::from_instance(self);

        if self.selected_content_type() == selected_type {
            return;
        }

        if selected_type == ContentType::None {
            priv_.content.navigate(adw::NavigationDirection::Back);
        } else {
            priv_.content.navigate(adw::NavigationDirection::Forward);
        }

        priv_.selected_content_type.set(selected_type);

        self.notify("selected-content-type");
    }

    pub fn selected_room(&self) -> Option<Room> {
        let priv_ = imp::Session::from_instance(self);
        priv_.selected_room.borrow().clone()
    }

    pub fn set_selected_room(&self, selected_room: Option<Room>) {
        let priv_ = imp::Session::from_instance(self);

        if self.selected_room() == selected_room {
            return;
        }

        priv_.selected_room.replace(selected_room);

        self.notify("selected-room");
    }

    pub fn login_with_password(&self, homeserver: Url, username: String, password: String) {
        let mut path = glib::user_data_dir();
        path.push(
            &Uuid::new_v4()
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer()),
        );

        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                let passphrase: String = {
                    let mut rng = thread_rng();
                    (&mut rng)
                        .sample_iter(Alphanumeric)
                        .take(30)
                        .map(char::from)
                        .collect()
                };
                let config = ClientConfig::new()
                    .request_config(RequestConfig::new().retry_limit(2))
                    .passphrase(passphrase.clone())
                    .store_path(path.clone());

                let client = Client::new_with_config(homeserver.clone(), config).unwrap();
                let response = client
                    .login(&username, &password, None, Some("Fractal Next"))
                    .await;
                match response {
                    Ok(response) => Ok((
                        client,
                        StoredSession {
                            homeserver: homeserver,
                            path: path,
                            passphrase: passphrase,
                            access_token: response.access_token,
                            user_id: response.user_id,
                            device_id: response.device_id,
                        },
                    )),
                    Err(error) => {
                        // Remove the store created by Client::new()
                        fs::remove_dir_all(path).unwrap();
                        Err(error)
                    }
                }
            },
            clone!(@weak self as obj => move |result| async move {
                obj.handle_login_result(result, true);
            }),
        );
    }

    pub fn room_search_bar(&self) -> gtk::SearchBar {
        let priv_ = imp::Session::from_instance(self);
        priv_.sidebar.room_search_bar()
    }

    pub fn login_with_previous_session(&self, session: StoredSession) {
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                let config = ClientConfig::new()
                    .request_config(RequestConfig::new().retry_limit(2))
                    .passphrase(session.passphrase.clone())
                    .store_path(session.path.clone());

                let client = Client::new_with_config(session.homeserver.clone(), config).unwrap();
                client
                    .restore_login(matrix_sdk::Session {
                        user_id: session.user_id.clone(),
                        device_id: session.device_id.clone(),
                        access_token: session.access_token.clone(),
                    })
                    .await
                    .map(|_| (client, session))
            },
            clone!(@weak self as obj => move |result| async move {
                obj.handle_login_result(result, false);
            }),
        );
    }

    fn handle_login_result(
        &self,
        result: Result<(Client, StoredSession), matrix_sdk::Error>,
        store_session: bool,
    ) {
        let priv_ = imp::Session::from_instance(self);
        match result {
            Ok((client, session)) => {
                priv_.client.set(client).unwrap();
                let user = User::new(self, &session.user_id);
                priv_.user.set(user).unwrap();

                if store_session {
                    // TODO: report secret service errors
                    secret::store_session(session).unwrap();
                }

                self.room_list().load();
                self.sync();
            }
            Err(error) => {
                priv_.error.replace(Some(error));
            }
        }
        self.emit_by_name("prepared", &[]).unwrap();
    }

    fn sync(&self) {
        let priv_ = imp::Session::from_instance(self);
        let sender = self.create_new_sync_response_sender();
        let client = priv_.client.get().unwrap().clone();
        RUNTIME.spawn(async move {
            // TODO: only create the filter once and reuse it in the future
            let room_event_filter = assign!(RoomEventFilter::default(), {
                lazy_load_options: LazyLoadOptions::Enabled {include_redundant_members: false},
            });
            let filter = assign!(FilterDefinition::default(), {
                room: assign!(RoomFilter::empty(), {
                    include_leave: true,
                    state: room_event_filter,
                }),
            });

            let sync_settings = SyncSettings::new()
                .timeout(Duration::from_secs(30))
                .filter(filter.into());
            client
                .sync_with_callback(sync_settings, |response| {
                    let sender = sender.clone();
                    async move {
                        // Using the event handler doesn't make a lot of sense for us since we want every room event
                        // Eventually we should contribute a better EventHandler interface so that it makes sense to use it.
                        send!(sender, response);

                        matrix_sdk::LoopCtrl::Continue
                    }
                })
                .await;
        });
    }

    fn mark_ready(&self) {
        let priv_ = &imp::Session::from_instance(self);
        priv_.stack.set_visible_child(&*priv_.content);
        priv_.is_ready.set(true).unwrap();
    }

    fn is_ready(&self) -> bool {
        let priv_ = &imp::Session::from_instance(self);
        priv_.is_ready.get().copied().unwrap_or_default()
    }

    pub fn room_list(&self) -> &RoomList {
        let priv_ = &imp::Session::from_instance(self);
        priv_.room_list.get_or_init(|| RoomList::new(self))
    }

    pub fn user(&self) -> &User {
        let priv_ = &imp::Session::from_instance(self);
        priv_.user.get().unwrap()
    }

    pub fn client(&self) -> &Client {
        let priv_ = &imp::Session::from_instance(self);
        priv_.client.get().unwrap()
    }

    /// Sets up the required channel to receive new room events
    fn create_new_sync_response_sender(&self) -> SyncSender<SyncResponse> {
        let (sender, receiver) =
            glib::MainContext::sync_channel::<SyncResponse>(Default::default(), 100);
        receiver.attach(
            None,
            clone!(@weak self as obj => @default-return glib::Continue(false), move |response| {
                if !obj.is_ready() {
                        obj.mark_ready();
                }
                obj.handle_sync_response(response);

                glib::Continue(true)
            }),
        );

        sender
    }

    /// This appends a new error to the list of errors
    pub fn append_error(&self, error: &Error) {
        let priv_ = imp::Session::from_instance(self);
        priv_.error_list.append(error);
    }

    /// Returns and consumes the `error` that was generated when the session failed to login,
    /// on a successful login this will be `None`.
    /// Unfortunately it's not possible to connect the Error directly to the `prepared` signals.
    pub fn get_error(&self) -> Option<matrix_sdk::Error> {
        let priv_ = &imp::Session::from_instance(self);
        priv_.error.take()
    }

    pub fn connect_prepared<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("prepared", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();

            f(&obj);

            None
        })
        .unwrap()
    }

    fn handle_sync_response(&self, response: SyncResponse) {
        self.room_list().handle_response_rooms(response.rooms);
    }
}
