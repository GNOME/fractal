use gtk::{
    gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate, ListView, SelectionModel,
};

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/sidebar-account-switcher.ui")]
    pub struct AccountSwitcher {
        #[template_child]
        pub users: TemplateChild<ListView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountSwitcher {
        const NAME: &'static str = "AccountSwitcher";
        type Type = super::AccountSwitcher;
        type ParentType = gtk::Popover;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AccountSwitcher {}
    impl WidgetImpl for AccountSwitcher {}
    impl PopoverImpl for AccountSwitcher {}
}

glib::wrapper! {
    pub struct AccountSwitcher(ObjectSubclass<imp::AccountSwitcher>)
        @extends gtk::Widget, gtk::Popover, @implements gtk::Accessible, gio::ListModel;
}

impl AccountSwitcher {
    pub fn set_logged_in_users(&self, main_stack_pages: &SelectionModel) {
        let users = imp::AccountSwitcher::from_instance(self).users.get();
        // let filtered_pages = gtk::SliceListModel::new(Some(main_stack_pages), 1, u32::MAX - 1);
        // let logged_in_users = gtk::SingleSelection::new(Some(&filtered_pages));

        users.set_model(Some(main_stack_pages));
    }
}
