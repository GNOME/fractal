use adw::{prelude::*, subclass::prelude::*};
use gtk::{glib, prelude::*, subclass::prelude::*};

use crate::session::sidebar::{CategoryRow, RoomRow};
use crate::session::{categories::Category, room::Room};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    pub struct Row {
        pub list_row: RefCell<Option<gtk::TreeListRow>>,
        pub binding: RefCell<Option<glib::Binding>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "SidebarRow";
        type Type = super::Row;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for Row {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "item",
                        "Item",
                        "The sidebar item of this row",
                        glib::Object::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpec::new_object(
                        "list-row",
                        "List Row",
                        "The list row to track for expander state",
                        gtk::TreeListRow::static_type(),
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
                "list-row" => {
                    let list_row = value.get().unwrap();
                    obj.set_list_row(list_row);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "item" => obj.item().to_value(),
                "list-row" => obj.list_row().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Row {}
    impl BinImpl for Row {}
}

glib::wrapper! {
    pub struct Row(ObjectSubclass<imp::Row>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl Row {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Row")
    }

    pub fn item(&self) -> Option<glib::Object> {
        self.list_row().and_then(|r| r.item())
    }

    pub fn list_row(&self) -> Option<gtk::TreeListRow> {
        let priv_ = imp::Row::from_instance(&self);
        priv_.list_row.borrow().clone()
    }

    pub fn set_list_row(&self, list_row: Option<gtk::TreeListRow>) {
        let priv_ = imp::Row::from_instance(&self);

        if self.list_row() == list_row {
            return;
        }

        if let Some(binding) = priv_.binding.take() {
            binding.unbind();
        }

        let row = if let Some(row) = list_row.clone() {
            priv_.list_row.replace(list_row.clone());
            row
        } else {
            return;
        };

        if let Some(item) = self.item() {
            if let Some(category) = item.downcast_ref::<Category>() {
                let child =
                    if let Some(Ok(child)) = self.child().map(|w| w.downcast::<CategoryRow>()) {
                        child
                    } else {
                        let child = CategoryRow::new();
                        self.set_child(Some(&child));
                        child
                    };
                child.set_category(Some(category.clone()));

                let binding = row
                    .bind_property("expanded", &child, "expanded")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build()
                    .unwrap();

                priv_.binding.replace(Some(binding));

                if let Some(list_item) = self.parent() {
                    list_item.set_css_classes(&["category"]);
                }
            } else if let Some(room) = item.downcast_ref::<Room>() {
                let child = if let Some(Ok(child)) = self.child().map(|w| w.downcast::<RoomRow>()) {
                    child
                } else {
                    let child = RoomRow::new();
                    self.set_child(Some(&child));
                    child
                };

                child.set_room(Some(room.clone()));

                if let Some(list_item) = self.parent() {
                    list_item.set_css_classes(&["room"]);
                }
            } else {
                panic!("Wrong row item: {:?}", item);
            }
        }

        self.notify("item");
        self.notify("list-row");
    }
}