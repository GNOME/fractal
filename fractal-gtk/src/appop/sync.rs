use crate::i18n::i18n;

use crate::appop::AppOp;

use crate::backend::BKCommand;

impl AppOp {
    pub fn initial_sync(&self, show: bool) {
        if show {
            self.inapp_notify(&i18n("Syncing, this could take a while"));
        } else {
            self.hide_inapp_notify();
        }
    }

    pub fn sync(&mut self, initial: bool, number_tries: u64) {
        if let (Some(login_data), true) = (self.login_data.clone(), !self.syncing) {
            self.syncing = true;
            // for the initial sync we set the since to None to avoid long syncing
            // the since can be a very old value and following the spec we should
            // do the initial sync without a since:
            // https://matrix.org/docs/spec/client_server/latest.html#syncing
            let since = if initial { None } else { self.since.clone() };
            self.backend
                .send(BKCommand::Sync(
                    login_data.server_url,
                    login_data.access_token,
                    login_data.uid,
                    since,
                    initial,
                    number_tries,
                ))
                .unwrap();
        }
    }

    pub fn synced(&mut self, since: Option<String>) {
        self.syncing = false;
        self.since = since;
        self.sync(false, 0);
        self.initial_sync(false);
    }

    pub fn sync_error(&mut self, number_tries: u64) {
        self.syncing = false;
        self.sync(false, number_tries);
    }
}
