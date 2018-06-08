extern crate gtk;
use self::gtk::prelude::*;

use app::App;

use libhandy;
use libhandy::ColumnExt;

impl App {
    /* create a width constrained column and add message_box to the UI */
    pub fn create_message_column(&self) {
        let container = self.ui.builder
            .get_object::<gtk::Box>("message_column")
            .expect("Can't find message_column in ui file.");
        let messages = self.op.lock().unwrap().message_box.clone();
        let column = libhandy::Column::new();
        column.set_maximum_width(800);
        /* For some reason the Column is not seen as a gtk::container
         * and therefore we can't call add() without the cast */
        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<gtk::Container>().unwrap();
        column.set_hexpand(true);
        column.set_vexpand(true);
        column.add(&messages);
        column.show();

        messages.get_style_context().unwrap().add_class("messages-history");
        messages.show();
        self.ui.builder.expose_object::<gtk::ListBox>("message_list", &messages);

        container.get_style_context().unwrap().add_class("messages-box");
        container.add(&column);
    }
}
