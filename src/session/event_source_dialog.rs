use crate::session::room::Event;
use adw::subclass::prelude::*;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use sourceview::prelude::*;

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/event-source-dialog.ui")]
    pub struct EventSourceDialog {
        pub event: OnceCell<Event>,
        #[template_child]
        pub source_view: TemplateChild<sourceview::View>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EventSourceDialog {
        const NAME: &'static str = "EventSourceDialog";
        type Type = super::EventSourceDialog;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.install_action("event-source-dialog.copy", None, move |widget, _, _| {
                widget.copy_to_clipboard();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for EventSourceDialog {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "event",
                    "Event",
                    "The event that is displayed in the Dialog",
                    Event::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "event" => {
                    let event = value.get().unwrap();
                    let _ = self.event.set(event);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "event" => self.event.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            let buffer = self
                .source_view
                .buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap();

            let md_lang = sourceview::LanguageManager::default().and_then(|lm| lm.language("json"));
            buffer.set_language(md_lang.as_ref());

            self.parent_constructed(obj);
        }
    }

    impl WidgetImpl for EventSourceDialog {}
    impl WindowImpl for EventSourceDialog {}
    impl AdwWindowImpl for EventSourceDialog {}
}

glib::wrapper! {
    pub struct EventSourceDialog(ObjectSubclass<imp::EventSourceDialog>)
        @extends gtk::Widget, gtk::Window, adw::Window, @implements gtk::Accessible;
}

impl EventSourceDialog {
    pub fn new(window: &gtk::Window, event: &Event) -> Self {
        glib::Object::new(&[("transient-for", window), ("event", event)])
            .expect("Failed to create EventSourceDialog")
    }

    pub fn copy_to_clipboard(&self) {
        let priv_ = imp::EventSourceDialog::from_instance(self);

        let clipboard = self.clipboard();
        let buffer = priv_.source_view.buffer();
        let (start_iter, end_iter) = buffer.bounds();
        clipboard.set_text(buffer.text(&start_iter, &end_iter, true).as_ref());
    }
}
