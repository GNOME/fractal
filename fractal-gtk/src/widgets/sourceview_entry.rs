use gtk::{self, prelude::*};
use libhandy::{Column, ColumnExt};
use sourceview::{self, ViewExt};

#[derive(Debug, Clone)]
pub struct SVEntry {
    pub column: Column,
    pub container: gtk::Box,
    pub attach: gtk::Button,
    pub markdown: gtk::MenuButton,
    pub markdown_img: gtk::Image,
    pub entry_box: gtk::Box,
    pub scroll: gtk::ScrolledWindow,
    pub view: sourceview::View,
    pub buffer: sourceview::Buffer,
}

impl Default for SVEntry {
    fn default() -> Self {
        let column = Column::new();
        column.set_maximum_width(800);
        column.set_linear_growth_width(600);
        /* For some reason the Column is not seen as a gtk::container
         * and therefore we can't call add() without the cast */
        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<gtk::Container>().unwrap();
        column.set_vexpand(false);

        let container = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        container.set_vexpand(false);

        let size = gtk::IconSize::Button.into();
        let attach = gtk::Button::new();
        let attach_img = gtk::Image::new_from_icon_name("mail-attachment-symbolic", size);
        attach.set_image(&attach_img);
        attach.set_valign(gtk::Align::End);
        attach.set_receives_default(true);
        attach.set_action_name("app.send-file");
        // TODO: there was an a11y object in the xml
        /*
        <object class="AtkObject" id="attach_button-atkobject">
          <property name="AtkObject::accessible-name" translatable="yes">Attach files</property>
        </object>
        */

        let markdown = gtk::MenuButton::new();
        let markdown_img = gtk::Image::new_from_icon_name("format-justify-left-symbolic", size);
        markdown.set_image(&markdown_img);
        markdown.set_valign(gtk::Align::End);
        markdown.set_receives_default(true);
        // TODO: there was an a11y object in the xml
        /*
        <object class="AtkObject" id="a11y-markdown_button">
          <property name="AtkObject::accessible_name" translatable="yes">Text formatting</property>
        </object>
        */

        let entry_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        entry_box
            .get_style_context()
            .map(|c| c.add_class("message-input"));

        let scroll = gtk::ScrolledWindow::new(None, None);

        let buffer = sourceview::Buffer::new(None);
        let view = sourceview::View::new_with_buffer(&buffer);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_indent_on_tab(false);

        scroll.add(&view);
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::External);
        scroll.set_max_content_height(100);
        scroll.set_propagate_natural_height(true);
        entry_box.add(&scroll);

        container.pack_start(&attach, false, false, 0);
        container.pack_start(&markdown, false, false, 0);
        container.pack_start(&entry_box, false, true, 0);

        column.add(&container);
        column.show_all();

        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<Column>().unwrap();

        SVEntry {
            column,
            container,
            attach,
            markdown,
            markdown_img,
            entry_box,
            scroll,
            view,
            buffer,
        }
    }
}
