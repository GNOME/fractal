use gdk;
use gdk::prelude::*;
use gtk;
use gtk::prelude::*;

use crate::uitypes::RowType;

#[derive(Clone)]
struct Widgets {
    popover: gtk::Popover,
    reply_button: gtk::ModelButton,
    open_with_button: gtk::ModelButton,
    save_image_as_button: gtk::ModelButton,
    save_video_as_button: gtk::ModelButton,
    copy_image_button: gtk::ModelButton,
    delete_message_button: gtk::ModelButton,
    view_source_button: gtk::ModelButton,
    copy_text_button: gtk::ModelButton,
    copy_selected_button: gtk::ModelButton,
    menu_separator: gtk::Widget,
}

impl Widgets {
    pub fn new(id: &str, mtype: &RowType, redactable: &bool) -> Widgets {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/message_menu.ui")
            .expect("Can't load ui file: message_menu.ui");

        let popover: gtk::Popover = builder
            .get_object("message_menu_popover")
            .expect("Can't find message_menu_popover in ui file.");

        let reply_button: gtk::ModelButton = builder
            .get_object("reply_button")
            .expect("Can't find reply_button in ui file.");

        let open_with_button: gtk::ModelButton = builder
            .get_object("open_with_button")
            .expect("Can't find open_with_button in ui file.");

        let save_image_as_button: gtk::ModelButton = builder
            .get_object("save_image_as_button")
            .expect("Can't find save_image_as_button in ui file.");

        let save_video_as_button: gtk::ModelButton = builder
            .get_object("save_video_as_button")
            .expect("Can't find save_video_as_button in ui file.");

        let copy_image_button: gtk::ModelButton = builder
            .get_object("copy_image_button")
            .expect("Can't find copy_image_button in ui file.");

        let copy_text_button: gtk::ModelButton = builder
            .get_object("copy_text_button")
            .expect("Can't find copy_text_button in ui file.");

        let delete_message_button: gtk::ModelButton = builder
            .get_object("delete_message_button")
            .expect("Can't find delete_message_button in ui file.");

        let view_source_button: gtk::ModelButton = builder
            .get_object("view_source_button")
            .expect("Can't find view_source_button in ui file.");

        let copy_selected_button: gtk::ModelButton = builder
            .get_object("copy_selected_text_button")
            .expect("Can't find copy_selected_text_button in ui file.");

        let menu_separator: gtk::Widget = builder
            .get_object("message_menu_separator")
            .expect("Can't find message_menu_separator");

        /* Set visibility of buttons */
        copy_selected_button.hide();
        delete_message_button.set_visible(*redactable);
        menu_separator.set_visible(*redactable);
        open_with_button.set_visible(mtype == &RowType::Image || mtype == &RowType::Video);
        save_image_as_button.set_visible(mtype == &RowType::Image);
        save_video_as_button.set_visible(mtype == &RowType::Video);
        copy_image_button.set_visible(mtype == &RowType::Image);
        copy_text_button.set_visible(mtype != &RowType::Image && mtype != &RowType::Video);

        let data = glib::Variant::from(id);
        reply_button.set_action_target_value(Some(&data));
        open_with_button.set_action_target_value(Some(&data));
        view_source_button.set_action_target_value(Some(&data));
        delete_message_button.set_action_target_value(Some(&data));
        open_with_button.set_action_target_value(Some(&data));
        save_image_as_button.set_action_target_value(Some(&data));
        save_video_as_button.set_action_target_value(Some(&data));
        copy_image_button.set_action_target_value(Some(&data));
        copy_text_button.set_action_target_value(Some(&data));

        Widgets {
            popover,
            reply_button,
            open_with_button,
            save_image_as_button,
            save_video_as_button,
            copy_image_button,
            delete_message_button,
            view_source_button,
            copy_text_button,
            copy_selected_button,
            menu_separator,
        }
    }
}

struct SelectedText {
    pub widget: glib::WeakRef<gtk::Label>,
    pub text: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Clone)]
pub struct MessageMenu {
    widgets: Widgets,
}

impl MessageMenu {
    pub fn new(
        id: &str,
        mtype: &RowType,
        redactable: &bool,
        widget: &gtk::EventBox,
        label: &gtk::Widget,
    ) -> MessageMenu {
        let menu = MessageMenu {
            widgets: Widgets::new(id, mtype, redactable),
        };
        /* Copy selected text works a little different then the other actions, because it need the
         * label */
        menu.connect_copy_selected_text(label);
        menu.show(widget);
        menu
    }

    fn show(&self, w: &gtk::EventBox) {
        gdk::Display::get_default()
            .and_then(|disp| disp.get_default_seat())
            .and_then(|seat| seat.get_pointer())
            .map(|ptr| {
                let win = w.get_window()?;
                let (_, x, y, _) = win.get_device_position(&ptr);

                let rect = gtk::Rectangle {
                    x,
                    y,
                    width: 0,
                    height: 0,
                };

                self.widgets.popover.set_relative_to(Some(w));
                self.widgets.popover.set_pointing_to(&rect);
                self.widgets.popover.set_position(gtk::PositionType::Bottom);

                self.widgets.popover.popup();

                Some(true)
            });
    }

    /* This should also be a action, but for some reason we need to set again the selection on the
     * label after the click event */
    fn connect_copy_selected_text(&self, w: &gtk::Widget) -> Option<()> {
        let label = w.downcast_ref::<gtk::Label>();
        let s = get_selected_text(label)?;
        self.widgets.copy_selected_button.show();
        self.widgets.copy_selected_button.connect_clicked(move |_| {
            let widget = upgrade_weak!(&s.widget);
            let atom = gdk::Atom::intern("CLIPBOARD");
            let clipboard = gtk::Clipboard::get(&atom);
            clipboard.set_text(&s.text);
            /* FIXME: for some reason we have to set the selection again */
            widget.select_region(s.start, s.end);
        });
        None
    }
}

fn get_selected_text(event_widget: Option<&gtk::Label>) -> Option<SelectedText> {
    let w = event_widget?;
    match w.get_selection_bounds() {
        Some((s, e)) => {
            let text = w.get_text()?;
            let slice: String = text.chars().take(e as usize).skip(s as usize).collect();
            Some(SelectedText {
                widget: w.downgrade(),
                text: slice,
                start: s,
                end: e,
            })
        }
        _ => None,
    }
}
