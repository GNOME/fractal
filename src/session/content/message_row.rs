use crate::components::Avatar;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{
    gio, glib, glib::clone, glib::signal::SignalHandlerId, prelude::*, subclass::prelude::*,
    CompositeTemplate,
};
use html2pango::{
    block::{markup_html, HtmlBlock},
    html_escape, markup_links,
};
use log::warn;
use matrix_sdk::ruma::events::{
    room::message::{FormattedBody, MessageFormat, MessageType, Relation},
    room::redaction::RedactionEventContent,
    AnyMessageEvent, AnyMessageEventContent, AnyRoomEvent,
};
use sourceview::prelude::*;

use crate::session::room::Event;

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-message-row.ui")]
    pub struct MessageRow {
        #[template_child]
        pub avatar: TemplateChild<Avatar>,
        #[template_child]
        pub header: TemplateChild<gtk::Box>,
        #[template_child]
        pub display_name: TemplateChild<gtk::Label>,
        #[template_child]
        pub timestamp: TemplateChild<gtk::Label>,
        #[template_child]
        pub content: TemplateChild<adw::Bin>,
        pub relates_to_changed_handler: RefCell<Option<SignalHandlerId>>,
        pub bindings: RefCell<Vec<glib::Binding>>,
        pub event: RefCell<Option<Event>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageRow {
        const NAME: &'static str = "ContentMessageRow";
        type Type = super::MessageRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Avatar::static_type();
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boolean(
                        "show-header",
                        "Show Header",
                        "Whether this item should show a header. This does nothing if this event doesn’t have a header. ",
                        false,
                        glib::ParamFlags::READWRITE,
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
                "show-header" => {
                    let show_header = value.get().unwrap();
                    let _ = obj.set_show_header(show_header);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "show-header" => obj.show_header().to_value(),
                _ => unimplemented!(),
            }
        }
    }
    impl WidgetImpl for MessageRow {}
    impl BinImpl for MessageRow {}
}

glib::wrapper! {
    pub struct MessageRow(ObjectSubclass<imp::MessageRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

//TODO
// - [] Implement widgets to show message events
impl MessageRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageRow")
    }

    pub fn show_header(&self) -> bool {
        let priv_ = imp::MessageRow::from_instance(self);
        priv_.avatar.is_visible() && priv_.header.is_visible()
    }

    pub fn set_show_header(&self, visible: bool) {
        let priv_ = imp::MessageRow::from_instance(self);
        priv_.avatar.set_visible(visible);
        priv_.header.set_visible(visible);

        if let Some(list_item) = self.parent().and_then(|w| w.parent()) {
            if visible {
                list_item.set_css_classes(&["has-header"]);
            } else {
                list_item.remove_css_class("has-header");
            }
        }

        self.notify("show-header");
    }

    pub fn set_event(&self, event: Event) {
        let priv_ = imp::MessageRow::from_instance(self);
        // Remove signals and bindings from the previous event
        if let Some(event) = priv_.event.take() {
            if let Some(relates_to_changed_handler) = priv_.relates_to_changed_handler.take() {
                event.disconnect(relates_to_changed_handler);
            }

            while let Some(binding) = priv_.bindings.borrow_mut().pop() {
                binding.unbind();
            }
        }

        priv_.avatar.set_item(Some(event.sender().avatar().clone()));

        let display_name_binding = event
            .sender()
            .bind_property("display-name", &priv_.display_name.get(), "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build()
            .unwrap();

        let show_header_binding = event
            .bind_property("show-header", self, "show-header")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build()
            .unwrap();

        let timestamp_binding = event
            .bind_property("time", &*priv_.timestamp, "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build()
            .unwrap();

        priv_.bindings.borrow_mut().append(&mut vec![
            display_name_binding,
            show_header_binding,
            timestamp_binding,
        ]);

        priv_
            .relates_to_changed_handler
            .replace(Some(event.connect_relates_to_changed(
                clone!(@weak self as obj => move |event| {
                    obj.update_content(&event);
                }),
            )));
        self.update_content(&event);
        priv_.event.replace(Some(event));
    }

    fn find_last_event(&self, event: &Event) -> Event {
        if let Some(replacement_event) = event.relates_to().iter().rev().find(|event| {
            let matrix_event = event.matrix_event();
            match matrix_event {
                AnyRoomEvent::Message(AnyMessageEvent::RoomMessage(message)) => {
                    message.content.relates_to.is_some()
                }
                AnyRoomEvent::Message(AnyMessageEvent::RoomRedaction(_)) => true,
                _ => false,
            }
        }) {
            if !replacement_event.relates_to().is_empty() {
                self.find_last_event(replacement_event)
            } else {
                replacement_event.clone()
            }
        } else {
            event.clone()
        }
    }
    /// Find the content we need to display
    fn find_content(&self, event: &Event) -> AnyMessageEventContent {
        match self.find_last_event(event).matrix_event() {
            AnyRoomEvent::Message(message) => message.content(),
            AnyRoomEvent::RedactedMessage(message) => {
                if let Some(ref redaction_event) = message.unsigned().redacted_because {
                    AnyMessageEventContent::RoomRedaction(redaction_event.content.clone())
                } else {
                    AnyMessageEventContent::RoomRedaction(RedactionEventContent::new())
                }
            }
            AnyRoomEvent::RedactedState(state) => {
                if let Some(ref redaction_event) = state.unsigned().redacted_because {
                    AnyMessageEventContent::RoomRedaction(redaction_event.content.clone())
                } else {
                    AnyMessageEventContent::RoomRedaction(RedactionEventContent::new())
                }
            }
            _ => panic!("This event isn’t a room message event or redacted event"),
        }
    }

    fn update_content(&self, event: &Event) {
        let priv_ = imp::MessageRow::from_instance(self);
        let content = self.find_content(event);

        // TODO: create widgets for all event types
        // TODO: display reaction events from event.relates_to()
        match content {
            AnyMessageEventContent::RoomMessage(message) => {
                let msgtype = if let Some(Relation::Replacement(replacement)) = message.relates_to {
                    replacement.new_content.msgtype
                } else {
                    message.msgtype
                };
                match msgtype {
                    MessageType::Audio(_message) => {}
                    MessageType::Emote(message) => {
                        let text = if let Some(formatted) = message
                            .formatted
                            .filter(|m| m.format == MessageFormat::Html)
                        {
                            markup_links(&html_escape(&formatted.body))
                        } else {
                            message.body
                        };
                        // TODO we need to bind the display name to the sender
                        self.show_label_with_markup(&format!(
                            "<b>{}</b> {}",
                            event.sender().display_name(),
                            text
                        ));
                    }
                    MessageType::File(_message) => {}
                    MessageType::Image(_message) => {}
                    MessageType::Location(_message) => {}
                    MessageType::Notice(message) => {
                        // TODO: we should reuse the already present child widgets when possible
                        let child = if let Some(html_blocks) =
                            parse_formatted_body(message.formatted.as_ref())
                        {
                            create_widget_for_html_message(html_blocks)
                        } else {
                            let child = gtk::Label::new(Some(&message.body));
                            set_label_styles(&child);
                            child.upcast::<gtk::Widget>()
                        };

                        priv_.content.set_child(Some(&child));
                    }
                    MessageType::ServerNotice(message) => {
                        self.show_label_with_text(&message.body);
                    }
                    MessageType::Text(message) => {
                        // TODO: we should reuse the already present child widgets when possible
                        let child = if let Some(html_blocks) =
                            parse_formatted_body(message.formatted.as_ref())
                        {
                            create_widget_for_html_message(html_blocks)
                        } else {
                            let child = gtk::Label::new(Some(&message.body));
                            set_label_styles(&child);
                            child.upcast::<gtk::Widget>()
                        };

                        priv_.content.set_child(Some(&child));
                    }
                    MessageType::Video(_message) => {}
                    MessageType::VerificationRequest(_message) => {}
                    _ => {
                        warn!("Event not supported: {:?}", msgtype)
                    }
                }
            }
            AnyMessageEventContent::RoomRedaction(_) => {
                self.show_label_with_text("This message was removed.");
            }
            _ => warn!("Event not supported: {:?}", content),
        }
    }

    fn show_label_with_text(&self, text: &str) {
        let priv_ = imp::MessageRow::from_instance(self);
        if let Some(Ok(child)) = priv_.content.child().map(|w| w.downcast::<gtk::Label>()) {
            child.set_text(&text);
        } else {
            let child = gtk::Label::new(Some(&text));
            set_label_styles(&child);
            priv_.content.set_child(Some(&child));
        }
    }

    fn show_label_with_markup(&self, text: &str) {
        let priv_ = imp::MessageRow::from_instance(self);
        if let Some(Ok(child)) = priv_.content.child().map(|w| w.downcast::<gtk::Label>()) {
            child.set_markup(&text);
        } else {
            let child = gtk::Label::new(None);
            child.set_markup(&text);
            set_label_styles(&child);
            priv_.content.set_child(Some(&child));
        }
    }
}

fn parse_formatted_body(formatted: Option<&FormattedBody>) -> Option<Vec<HtmlBlock>> {
    formatted
        .filter(|m| m.format == MessageFormat::Html)
        .filter(|formatted| !formatted.body.contains("<!-- raw HTML omitted -->"))
        .and_then(|formatted| markup_html(&formatted.body).ok())
}

fn create_widget_for_html_message(blocks: Vec<HtmlBlock>) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    for block in blocks {
        let widget = create_widget_for_html_block(&block);
        container.append(&widget);
    }
    container.upcast::<gtk::Widget>()
}

fn set_label_styles(w: &gtk::Label) {
    w.set_wrap(true);
    w.set_justify(gtk::Justification::Left);
    w.set_xalign(0.0);
    w.set_valign(gtk::Align::Start);
    w.set_halign(gtk::Align::Fill);
    w.set_selectable(true);
    let menu_model: Option<gio::MenuModel> =
        gtk::Builder::from_resource("/org/gnome/FractalNext/content-item-row-menu.ui")
            .object("menu_model");
    w.set_extra_menu(menu_model.as_ref());
}

fn create_widget_for_html_block(block: &HtmlBlock) -> gtk::Widget {
    match block {
        HtmlBlock::Heading(n, s) => {
            let w = gtk::Label::new(None);
            set_label_styles(&w);
            w.set_markup(&s);
            w.add_css_class(&format!("h{}", n));
            w.upcast::<gtk::Widget>()
        }
        HtmlBlock::UList(elements) => {
            let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);
            bx.set_margin_end(6);
            bx.set_margin_start(6);

            for li in elements.iter() {
                let h_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let bullet = gtk::Label::new(Some("•"));
                bullet.set_valign(gtk::Align::Start);
                let w = gtk::Label::new(None);
                set_label_styles(&w);
                h_box.append(&bullet);
                h_box.append(&w);
                w.set_markup(&li);
                bx.append(&h_box);
            }

            bx.upcast::<gtk::Widget>()
        }
        HtmlBlock::OList(elements) => {
            let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);
            bx.set_margin_end(6);
            bx.set_margin_start(6);

            for (i, ol) in elements.iter().enumerate() {
                let h_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let bullet = gtk::Label::new(Some(&format!("{}.", i + 1)));
                bullet.set_valign(gtk::Align::Start);
                let w = gtk::Label::new(None);
                set_label_styles(&w);
                h_box.append(&bullet);
                h_box.append(&w);
                w.set_markup(&ol);
                bx.append(&h_box);
            }

            bx.upcast::<gtk::Widget>()
        }
        HtmlBlock::Code(s) => {
            let scrolled = gtk::ScrolledWindow::new();
            scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
            let buffer = sourceview::Buffer::new(None);
            buffer.set_highlight_matching_brackets(false);
            buffer.set_text(&s);
            let view = sourceview::View::with_buffer(&buffer);
            view.set_editable(false);
            view.add_css_class("codeview");
            scrolled.set_child(Some(&view));
            scrolled.upcast::<gtk::Widget>()
        }
        HtmlBlock::Quote(blocks) => {
            let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);
            bx.add_css_class("quote");
            for block in blocks.iter() {
                let w = create_widget_for_html_block(block);
                bx.append(&w);
            }
            bx.upcast::<gtk::Widget>()
        }
        HtmlBlock::Text(s) => {
            let w = gtk::Label::new(None);
            set_label_styles(&w);
            w.set_markup(&s);
            w.upcast::<gtk::Widget>()
        }
    }
}
