use gtk;
use gtk::prelude::*;
use sourceview4::prelude::*;

struct Widgets {
    msg_src_window: gtk::Window,
    copy_src_button: gtk::Button,
    close_src_button: gtk::Button,
    source_buffer: sourceview4::Buffer,
}

impl Widgets {
    pub fn new() -> Widgets {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/msg_src_window.ui")
            .expect("Can't load ui file: msg_src_window.ui");

        let msg_src_window: gtk::Window = builder
            .get_object("msg_src_window")
            .expect("Can't find msg_src_window in ui file.");

        let copy_src_button: gtk::Button = builder
            .get_object("copy_src_button")
            .expect("Can't find copy_src_button in ui file.");

        let close_src_button: gtk::Button = builder
            .get_object("close_src_button")
            .expect("Can't find close_src_button in ui file.");

        let source_buffer: sourceview4::Buffer = builder
            .get_object("source_buffer")
            .expect("Can't find source_buffer in ui file.");

        Widgets {
            msg_src_window,
            copy_src_button,
            close_src_button,
            source_buffer,
        }
    }
}

pub struct SourceDialog {
    widgets: Widgets,
}

impl SourceDialog {
    pub fn new() -> SourceDialog {
        let viewer = SourceDialog {
            widgets: Widgets::new(),
        };
        viewer.connect();
        viewer
    }

    pub fn show(&self, source: &str) {
        self.widgets.source_buffer.set_text(source);
        self.widgets.msg_src_window.show();
    }

    /* This sets the transient_for parent */
    pub fn set_parent_window(&self, parent: &gtk::Window) {
        self.widgets.msg_src_window.set_transient_for(Some(parent));
    }

    fn connect(&self) {
        let source_buffer = self.widgets.source_buffer.downgrade();
        self.widgets.copy_src_button.connect_clicked(move |_| {
            let source_buffer = upgrade_weak!(source_buffer);
            let atom = gdk::Atom::intern("CLIPBOARD");
            let clipboard = gtk::Clipboard::get(&atom);

            let start_iter = source_buffer.get_start_iter();
            let end_iter = source_buffer.get_end_iter();

            if let Some(src) = source_buffer.get_text(&start_iter, &end_iter, false) {
                clipboard.set_text(&src);
            }
        });

        let msg_src_window = self.widgets.msg_src_window.downgrade();
        self.widgets.close_src_button.connect_clicked(move |_| {
            upgrade_weak!(msg_src_window).close();
        });

        /* Close the window when the user preses ESC */
        self.widgets.msg_src_window.connect_key_press_event(|w, k| {
            if k.get_keyval() == gdk::enums::key::Escape {
                w.close();
            }

            Inhibit(true)
        });

        let json_lang =
            sourceview4::LanguageManager::get_default().map_or(None, |lm| lm.get_language("json"));

        self.widgets
            .source_buffer
            .set_highlight_matching_brackets(false);
        if let Some(ref json_lang) = json_lang {
            self.widgets.source_buffer.set_language(Some(json_lang));
            self.widgets.source_buffer.set_highlight_syntax(true);

            if let Some(scheme) = sourceview4::StyleSchemeManager::get_default()
                .map_or(None, |scm| scm.get_scheme("kate"))
            {
                self.widgets.source_buffer.set_style_scheme(Some(&scheme));
            }
        }
    }
}
