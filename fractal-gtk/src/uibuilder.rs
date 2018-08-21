extern crate gtk;
use gtk::prelude::*;

use uibuilder::gtk::BuilderExt;
use libhandy::{Column, ColumnExt};
use sourceview;
use sourceview::ViewExt;

#[derive(Clone)]
pub struct UI {
    pub builder: gtk::Builder,
}

impl UI {
    pub fn new() -> UI {
        // The order here is important because some ui file depends on others

        let builder = gtk::Builder::new();

        builder.add_from_resource("/org/gnome/Fractal/ui/autocomplete.ui")
               .expect("Can't load ui file: autocomplete.ui");

        // needed from main_window
        // These are popup menus showed from main_window interface
        builder.add_from_resource("/org/gnome/Fractal/ui/user_menu.ui")
               .expect("Can't load ui file: user_menu.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/add_room_menu.ui")
               .expect("Can't load ui file: add_room_menu.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/room_menu.ui")
               .expect("Can't load ui file: room_menu.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/members.ui")
               .expect("Can't load ui file: members.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/markdown_popover.ui")
               .expect("Can't load ui file: markdown_popover.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/server_chooser_menu.ui")
               .expect("Can't load ui file: server_chooser_menu.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/stickers_popover.ui")
               .expect("Can't load ui file: stickers_popover.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/media_viewer_menu.ui")
               .expect("Can't load ui file: media_viewer_menu.ui");

        builder.add_from_resource("/org/gnome/Fractal/ui/main_window.ui")
               .expect("Can't load ui file: main_window.ui");

        room_message_entry_reimpl(&builder);

        // Depends on main_window
        // These are all dialogs transient for main_window
        builder.add_from_resource("/org/gnome/Fractal/ui/direct_chat.ui")
               .expect("Can't load ui file: direct_chat.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/invite.ui")
               .expect("Can't load ui file: invite.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/invite_user.ui")
               .expect("Can't load ui file: invite_user.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/join_room.ui")
               .expect("Can't load ui file: join_room.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/leave_room.ui")
               .expect("Can't load ui file: leave_room.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/new_room.ui")
               .expect("Can't load ui file: new_room.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/password_dialog.ui")
               .expect("Can't load ui file: password_dialog.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/account_settings.ui")
               .expect("Can't load ui file: account_settings.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/media_viewer.ui")
               .expect("Can't load ui file: media_viewer.ui");
        builder.add_from_resource("/org/gnome/Fractal/ui/msg_src_window.ui")
               .expect("Can't load ui file: msg_src_window.ui");

        // Depends on room config
        /*
        builder.add_from_resource("/org/gnome/Fractal/ui/filechooser.ui")
               .expect("Can't load ui file: filechooser.ui");
               */

        UI { builder }
    }
}

fn room_message_entry_reimpl(builder: &gtk::Builder) {
    let column = Column::new();
    column.set_maximum_width(800);
    /* For some reason the Column is not seen as a gtk::container
     * and therefore we can't call add() without the cast */
    let column = column.upcast::<gtk::Widget>();
    let column = column.downcast::<gtk::Container>().unwrap();
    column.set_vexpand(false);

    let room_msg_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    room_msg_box.set_vexpand(false);

    let size = gtk::IconSize::Button.into();
    let attach = gtk::Button::new();
    let attach_img = gtk::Image::new_from_icon_name("mail-attachment-symbolic", size);
    attach.set_image(&attach_img);
    attach.set_valign(gtk::Align::End);
    attach.set_receives_default(true);
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

    let msg_entry_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    msg_entry_box.get_style_context().map(|c| c.add_class("message-input"));

    let scroll = gtk::ScrolledWindow::new(None, None);

    let buffer = sourceview::Buffer::new(None);
    let sv = sourceview::View::new_with_buffer(&buffer);
    sv.set_wrap_mode(gtk::WrapMode::WordChar);
    sv.set_indent_on_tab(false);

    scroll.add(&sv);
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::External);
    scroll.set_max_content_height(100);
    scroll.set_propagate_natural_height(true);
    msg_entry_box.add(&scroll);

    room_msg_box.pack_start(&attach, false, false, 0);
    room_msg_box.pack_start(&markdown, false, false, 0);
    room_msg_box.pack_start(&msg_entry_box, false, true, 0);

    let parent: gtk::Box = builder.get_object("room_parent").unwrap();
    column.add(&room_msg_box);
    parent.add(&column);
    column.show_all();

    // Keep compatibilit with the rest of the codebase
    builder.expose_object::<gtk::Box>("room_message_box", &room_msg_box);
    builder.expose_object::<gtk::Button>("attach_button", &attach);
    builder.expose_object::<gtk::MenuButton>("markdown_button", &markdown);
    builder.expose_object::<gtk::Image>("md_img", &markdown_img);
    builder.expose_object::<gtk::Box>("msg_entry_box", &msg_entry_box);
    builder.expose_object::<gtk::ScrolledWindow>("input_scroll", &scroll);
    builder.expose_object::<sourceview::View>("msg_entry", &sv);
    builder.expose_object::<sourceview::Buffer>("msg_entry_buffer", &buffer);
}
