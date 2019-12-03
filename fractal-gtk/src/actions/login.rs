use log::{debug, warn};
use std::cell::RefCell;
use std::rc::Rc;

use gio::prelude::*;
use gio::SimpleAction;
use gio::SimpleActionGroup;
use gtk::prelude::*;

use crate::globals;

#[derive(Debug, Clone, PartialEq)]
pub enum LoginState {
    Greeter,
    ServerChooser,
    Credentials,
}

impl From<String> for LoginState {
    fn from(v: String) -> LoginState {
        match v.as_str() {
            "greeter" => LoginState::Greeter,
            "server-chooser" => LoginState::ServerChooser,
            "credentials" => LoginState::Credentials,
            _ => panic!("Invalid back state type"),
        }
    }
}

impl ToString for LoginState {
    fn to_string(&self) -> String {
        let str = match self {
            LoginState::Greeter => "greeter",
            LoginState::ServerChooser => "server-chooser",
            LoginState::Credentials => "credentials",
        };

        String::from(str)
    }
}

pub fn new(
    stack: &gtk::Stack,
    headers: &gtk::Stack,
    server_entry: &gtk::Entry,
    err_label: &gtk::Label,
) -> SimpleActionGroup {
    let actions = SimpleActionGroup::new();

    let create_account = SimpleAction::new("create-account", None);
    let server_chooser = SimpleAction::new("server_chooser", None);
    let credentials = SimpleAction::new("credentials", None);
    let back = SimpleAction::new("back", None);
    let login = SimpleAction::new("login", None);

    actions.add_action(&create_account);
    actions.add_action(&server_chooser);
    actions.add_action(&credentials);
    actions.add_action(&back);
    actions.add_action(&login);

    let stack_weak = stack.downgrade();
    create_account.connect_activate(move |_, _| {
        let stack = upgrade_weak!(stack_weak);

        let toplevel = stack
            .get_toplevel()
            .expect("Could not grab toplevel widget")
            .downcast::<gtk::Window>()
            .expect("Could not cast toplevel to GtkWindow");
        let uri = globals::RIOT_REGISTER_URL;
        if let Err(e) = gtk::show_uri_on_window(Some(&toplevel), uri, gtk::get_current_event_time())
        {
            warn!("Could not show {}: {}", uri, e)
        }
    });

    let back_history: Rc<RefCell<Vec<LoginState>>> = Rc::new(RefCell::new(vec![]));

    let back_weak = Rc::downgrade(&back_history);
    let stack_weak = stack.downgrade();
    server_chooser.connect_activate(move |_, _| {
        let stack = upgrade_weak!(stack_weak);
        let back = upgrade_weak!(back_weak);

        let state = LoginState::ServerChooser;
        stack.set_visible_child_name(&state.to_string());
        back.borrow_mut().push(state);
    });

    let back_weak = Rc::downgrade(&back_history);
    let stack_weak = stack.downgrade();
    let server_weak = server_entry.downgrade();
    let err_weak = err_label.downgrade();
    credentials.connect_activate(move |_, _| {
        let stack = upgrade_weak!(stack_weak);
        let back = upgrade_weak!(back_weak);
        let server_entry = upgrade_weak!(server_weak);
        let err_label = upgrade_weak!(err_weak);

        server_entry.get_text().map(|txt| {
            if txt.is_empty() {
                err_label.show();
            } else {
                err_label.hide();
                let state = LoginState::Credentials;
                stack.set_visible_child_name(&state.to_string());
                back.borrow_mut().push(state);
            }
        });
    });

    let stack_weak = stack.downgrade();
    back.connect_activate(move |_, _| {
        let stack = upgrade_weak!(stack_weak);
        back_history.borrow_mut().pop();
        if let Some(state) = back_history.borrow().last() {
            debug!("Go back to state {}", state.to_string());
            stack.set_visible_child_name(&state.to_string());
        } else {
            debug!("There is no state to go back to. Go back to state greeter");
            stack.set_visible_child_name(&LoginState::Greeter.to_string());
        }
    });

    gio::Application::get_default().map(|app| {
        app.downcast::<gtk::Application>().map(|gtk_app| {
            gtk_app.get_active_window().map(|window| {
                window.connect_button_press_event(move |_, e| {
                    if e.get_button() == 8 {
                        back.activate(None);
                    }
                    Inhibit(false)
                });
            })
        })
    });

    stack.insert_action_group("login", Some(&actions));
    headers.insert_action_group("login", Some(&actions));

    actions
}
