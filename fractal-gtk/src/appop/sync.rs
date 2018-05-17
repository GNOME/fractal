extern crate gettextrs;
use self::gettextrs::gettext;

use appop::AppOp;

use backend::BKCommand;


impl AppOp {
    pub fn initial_sync(&self, show: bool) {
        if show {
            self.inapp_notify(&gettext("Syncing, this could take a while"));
        } else {
            self.hide_inapp_notify();
        }
    }

    pub fn sync(&mut self) {
        if !self.syncing && self.logged_in {
            self.syncing = true;
            self.backend.send(BKCommand::Sync).unwrap();
        }
    }

    pub fn synced(&mut self, since: Option<String>) {
        self.syncing = false;
        self.since = since;
        self.sync();
        self.initial_sync(false);
    }

    pub fn sync_error(&mut self) {
        self.syncing = false;
        self.sync();
    }
}
