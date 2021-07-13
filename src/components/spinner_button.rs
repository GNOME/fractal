use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

mod imp {
    use super::*;
    use glib::object::ObjectClass;
    use glib::subclass::InitializingObject;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/spinner-button.ui")]
    pub struct SpinnerButton {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpinnerButton {
        const NAME: &'static str = "SpinnerButton";
        type Type = super::SpinnerButton;
        type ParentType = gtk::Button;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SpinnerButton {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_override(
                        "label",
                        &ObjectClass::from_type(gtk::Button::static_type())
                            .unwrap()
                            .find_property("label")
                            .unwrap(),
                    ),
                    glib::ParamSpec::new_boolean(
                        "loading",
                        "Loading",
                        "Whether to display the loading spinner or the content",
                        false,
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
                "label" => obj.set_label(value.get().unwrap()),
                "loading" => obj.set_loading(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "label" => obj.label().to_value(),
                "loading" => obj.loading().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for SpinnerButton {}

    impl ButtonImpl for SpinnerButton {}
}

glib::wrapper! {
    pub struct SpinnerButton(ObjectSubclass<imp::SpinnerButton>)
        @extends gtk::Widget, gtk::Button, @implements gtk::Accessible, gtk::Actionable;
}

/// A widget displaying a `User`
impl SpinnerButton {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SpinnerButton")
    }

    pub fn set_label(&self, label: &str) {
        let priv_ = imp::SpinnerButton::from_instance(self);

        if priv_.label.label().as_str() == label {
            return;
        }

        priv_.label.set_label(label);

        self.notify("label");
    }

    pub fn label(&self) -> glib::GString {
        let priv_ = imp::SpinnerButton::from_instance(self);
        priv_.label.label()
    }

    pub fn set_loading(&self, loading: bool) {
        let priv_ = imp::SpinnerButton::from_instance(self);

        if self.loading() == loading {
            return;
        }

        self.set_sensitive(!loading);

        if loading {
            priv_.stack.set_visible_child(&*priv_.spinner);
        } else {
            priv_.stack.set_visible_child(&*priv_.label);
        }

        self.notify("loading");
    }

    pub fn loading(&self) -> bool {
        let priv_ = imp::SpinnerButton::from_instance(self);
        priv_.stack.visible_child().as_ref() == Some(priv_.spinner.upcast_ref())
    }
}
