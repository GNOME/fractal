use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use log::warn;
use matrix_sdk::ruma::events::{
    room::member::MembershipState, AnyStateEvent, AnyStateEventContent,
};

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-state-row.ui")]
    pub struct StateRow {
        #[template_child]
        pub timestamp: TemplateChild<gtk::Label>,
        #[template_child]
        pub content: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StateRow {
        const NAME: &'static str = "ContentStateRow";
        type Type = super::StateRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StateRow {}
    impl WidgetImpl for StateRow {}
    impl BinImpl for StateRow {}
}

glib::wrapper! {
    pub struct StateRow(ObjectSubclass<imp::StateRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

//TODO
// - [] Implement widgets to show state events
impl StateRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create StateRow")
    }

    pub fn update(&self, state: &AnyStateEvent) {
        let _priv_ = imp::StateRow::from_instance(self);
        // We may want to show more state events in the future
        // For a full list of state events see:
        // https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk/events/enum.AnyStateEventContent.html
        let message = match state.content() {
            AnyStateEventContent::RoomCreate(_event) => gettext("The beginning of this room."),
            AnyStateEventContent::RoomEncryption(_event) => gettext("This room is now encrypted."),
            AnyStateEventContent::RoomMember(event) => {
                let display_name = event
                    .displayname
                    .clone()
                    .unwrap_or(state.state_key().into());

                match event.membership {
                    MembershipState::Join => {
                        let message = match state.prev_content() {
                            Some(AnyStateEventContent::RoomMember(prev))
                                if event.membership != prev.membership =>
                            {
                                None
                            }
                            Some(AnyStateEventContent::RoomMember(prev))
                                if event.displayname != prev.displayname =>
                            {
                                if let Some(prev_name) = prev.displayname {
                                    if event.displayname == None {
                                        Some(gettext!("{} removed their display name.", prev_name))
                                    } else {
                                        Some(gettext!(
                                            "{} changed their display name to {}.",
                                            prev_name,
                                            display_name
                                        ))
                                    }
                                } else {
                                    Some(gettext!(
                                        "{} set their display name to {}.",
                                        state.state_key(),
                                        display_name
                                    ))
                                }
                            }
                            Some(AnyStateEventContent::RoomMember(prev))
                                if event.avatar_url != prev.avatar_url =>
                            {
                                if prev.avatar_url == None {
                                    Some(gettext!("{} set their avatar.", display_name))
                                } else if event.avatar_url == None {
                                    Some(gettext!("{} removed their avatar.", display_name))
                                } else {
                                    Some(gettext!("{} changed their avatar.", display_name))
                                }
                            }
                            _ => None,
                        };

                        message.unwrap_or(gettext!("{} joined this room.", display_name))
                    }
                    MembershipState::Invite => {
                        gettext!("{} was invited to this room.", display_name)
                    }
                    MembershipState::Knock => {
                        // TODO: Add button to invite the user.
                        gettext!("{} requested to be invited to this room.", display_name)
                    }
                    MembershipState::Leave => {
                        let message = match state.prev_content() {
                            Some(AnyStateEventContent::RoomMember(prev))
                                if prev.membership == MembershipState::Invite =>
                            {
                                if state.state_key() == state.sender() {
                                    Some(gettext!("{} rejected the invite.", display_name))
                                } else {
                                    Some(gettext!("{}’s invite was revoked'.", display_name))
                                }
                            }
                            Some(AnyStateEventContent::RoomMember(prev))
                                if prev.membership == MembershipState::Ban =>
                            {
                                Some(gettext!("{} was unbanned.", display_name))
                            }
                            _ => None,
                        };

                        message.unwrap_or_else(|| {
                            if state.state_key() == state.sender() {
                                gettext!("{} left the room.", display_name)
                            } else {
                                gettext!("{} was kicked out of the room.", display_name)
                            }
                        })
                    }
                    MembershipState::Ban => gettext!("{} was banned.", display_name),
                    _ => {
                        warn!("Unsupported room member event: {:?}", state);
                        gettext("An unsupported room member event was received.")
                    }
                }
            }
            AnyStateEventContent::RoomThirdPartyInvite(event) => {
                let display_name = match event.display_name {
                    s if s.is_empty() => state.state_key().into(),
                    s => s,
                };
                gettext!("{} was invited to this room.", display_name)
            }
            AnyStateEventContent::RoomTombstone(event) => {
                gettext!("The room was upgraded: {}", event.body)
                // Todo: add button for new room with action session.show_room::room_id
            }
            _ => {
                warn!("Unsupported state event: {}", state.event_type());
                gettext("An unsupported state event was received.")
            }
        };
        if let Some(Ok(child)) = self.child().map(|w| w.downcast::<gtk::Label>()) {
            child.set_text(&message);
        } else {
            let child = gtk::Label::new(Some(&message));
            child.set_css_classes(&["event-content", "dim-label"]);
            child.set_wrap(true);
            child.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            self.set_child(Some(&child));
        };
    }
}
