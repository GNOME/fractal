extern crate secret_service;
extern crate serde_json;

use gio::Settings;
use gio::SettingsExt;

use std;


#[derive(Debug)]
pub enum Error {
    SecretServiceError,
    PlainTextError,
}

derror!(secret_service::SsError, Error::SecretServiceError);
derror!(std::io::Error, Error::PlainTextError);
derror!(serde_json::Error, Error::PlainTextError);


enum PWDConf {
    SecretService,
    PlainText,
}


fn pwd_conf() -> PWDConf {
    if Settings::list_schemas().iter().filter(|x| &x[..] == "org.gnome.Fractal").count() == 0 {
        return PWDConf::SecretService;
    }

    let settings: Settings = Settings::new("org.gnome.Fractal");

    match settings.get_enum("password-storage") {
        1 => PWDConf::PlainText,
        _ => PWDConf::SecretService,
    }
}


pub trait PasswordStorage {
    fn delete_pass(&self, key: &str) -> Result<(), Error> {
        match pwd_conf() {
            PWDConf::PlainText => plain_text::delete_pass(key),
            _ => ss_storage::delete_pass(key),
        }
    }

    fn store_pass(&self, username: String, password: String, server: String, identity: String) -> Result<(), Error> {
        match pwd_conf() {
            PWDConf::PlainText => plain_text::store_pass(username, password, server, identity),
            _ => ss_storage::store_pass(username, password, server, identity),
        }
    }

    fn get_pass(&self) -> Result<(String, String, String, String), Error> {
        match pwd_conf() {
            PWDConf::PlainText => plain_text::get_pass(),
            _ => ss_storage::get_pass(),
        }
    }

    fn store_token(&self, uid: String, token: String) -> Result<(), Error> {
        match pwd_conf() {
            PWDConf::PlainText => plain_text::store_token(uid, token),
            _ => ss_storage::store_token(uid, token),
        }
    }

    fn get_token(&self) -> Result<(String, String), Error> {
        match pwd_conf() {
            PWDConf::PlainText => plain_text::get_token(),
            _ => ss_storage::get_token(),
        }
    }
}


mod ss_storage {
    use super::Error;

    use super::secret_service::SecretService;
    use super::secret_service::EncryptionType;

    pub fn delete_pass(key: &str) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;

        // deleting previous items
        let allpass = collection.get_all_items()?;
        let passwds = allpass
            .iter()
            .filter(|x| x.get_label().unwrap_or_default() == key);
        for p in passwds {
            p.unlock()?;
            p.delete()?;
        }

        Ok(())
    }

    pub fn store_token(uid: String, token: String) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let key = "fractal-token";

        // deleting previous items
        delete_pass(key)?;

        // create new item
        collection.unlock()?;
        collection.create_item(
            key,                 // label
            vec![("uid", &uid)], // properties
            token.as_bytes(),    //secret
            true,                // replace item with same attributes
            "text/plain",        // secret content type
        )?;

        Ok(())
    }

    pub fn get_token() -> Result<(String, String), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;
        let key = "fractal-token";

        let passwd = allpass
            .iter()
            .find(|x| x.get_label().unwrap_or_default() == key);

        if passwd.is_none() {
            return Err(Error::SecretServiceError);
        }

        let p = passwd.unwrap();
        p.unlock()?;
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;
        let token = String::from_utf8(secret).unwrap();

        let attr = attrs
            .iter()
            .find(|&ref x| x.0 == "uid")
            .ok_or(Error::SecretServiceError)?;
        let uid = attr.1.clone();

        Ok((token, uid))
    }

    pub fn store_pass(username: String, password: String, server: String, identity: String) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let key = "fractal";

        // deleting previous items
        delete_pass(key)?;

        // create new item
        collection.unlock()?;
        collection.create_item(
            key,                                                // label
            vec![("username", &username), ("server", &server), ("identity", &identity)], // properties
            password.as_bytes(),                                //secret
            true,                                               // replace item with same attributes
            "text/plain",                                       // secret content type
        )?;

        Ok(())
    }

    pub fn migrate_old_passwd() -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;

        // old name password
        let passwd = allpass
            .iter()
            .find(|x| x.get_label().unwrap_or(strn!("")) == "guillotine");

        if passwd.is_none() {
            return Ok(());
        }

        let p = passwd.unwrap();
        p.unlock()?;
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;

        let mut attr = attrs
            .iter()
            .find(|&ref x| x.0 == "username")
            .ok_or(Error::SecretServiceError)?;
        let username = attr.1.clone();
        attr = attrs
            .iter()
            .find(|&ref x| x.0 == "server")
            .ok_or(Error::SecretServiceError)?;
        let server = attr.1.clone();
        let pwd = String::from_utf8(secret).unwrap();

        // removing old
        for p in passwd {
            p.delete()?;
        }
        /* It wasn't possibile to have a different identity server therefore set it always to
         * vector.im */
        let identity = String::from("https://vector.im");

        store_pass(username, pwd, server, identity)?;

        Ok(())
    }

    pub fn get_pass() -> Result<(String, String, String, String), Error> {
        migrate_old_passwd()?;

        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;
        let key = "fractal";

        let passwd = allpass
            .iter()
            .find(|x| x.get_label().unwrap_or_default() == key);

        if passwd.is_none() {
            return Err(Error::SecretServiceError);
        }

        let p = passwd.unwrap();
        p.unlock()?;
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;

        let attr = attrs
            .iter()
            .find(|&ref x| x.0 == "username")
            .ok_or(Error::SecretServiceError)?;
        let username = attr.1.clone();
        let attr = attrs
            .iter()
            .find(|&ref x| x.0 == "server")
            .ok_or(Error::SecretServiceError)?;
        let server = attr.1.clone();

        let attr = attrs
            .iter()
            .find(|&ref x| x.0 == "identity");

        /* Fallback to the vector identity server when there is none */
        let identity = match attr {
            Some(a) => a.1.clone(),
            None => {
                String::from("https://vector.im")
            },
        };

        let tup = (username, String::from_utf8(secret).unwrap(), server, identity);

        Ok(tup)
    }
}


mod plain_text {
    use super::Error;
    use super::serde_json;
    use glib::get_user_config_dir;
    use std::fs::create_dir_all;
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::prelude::*;

    #[derive(Serialize, Deserialize, Default)]
    pub struct UserData {
        pub username: String,
        pub server: String,
        pub identity: String,
        pub password: Option<String>,
        pub token: Option<String>,
    }

    fn get_file(name: &str) -> Result<String, Error> {
        let mut path = match get_user_config_dir() {
            Some(path) => path,
            None => PathBuf::from("~"),
        };

        path.push("fractal");

        if !path.exists() {
            create_dir_all(&path)?;
        }

        path.push(name);
        Ok(path.into_os_string().into_string().unwrap_or_default())
    }

    fn load() -> Result<UserData, Error> {
        let fname = get_file(".fractal-userdata.json")?;
        let mut file = File::open(fname)?;
        let mut serialized = String::new();
        file.read_to_string(&mut serialized)?;
        let deserialized: UserData = serde_json::from_str(&serialized)?;

        Ok(deserialized)
    }

    fn store(data: &UserData) -> Result<(), Error> {
        let fname = get_file(".fractal-userdata.json")?;
        let serialized = serde_json::to_string(data)?;
        File::create(fname)?.write_all(&serialized.into_bytes())?;

        Ok(())
    }

    pub fn delete_pass(key: &str) -> Result<(), Error> {
        let mut data = load().unwrap_or_default();
        match key {
            "fractal" => { data.password = None; },
            "fractal-token" => { data.token = None; },
            _ => { data.password = None; },
        };
        store(&data)?;

        Ok(())
    }

    pub fn store_token(uid: String, token: String) -> Result<(), Error> {
        let mut data = load().unwrap_or_default();
        data.username = uid;
        data.token = Some(token);
        store(&data)?;
        Ok(())
    }

    pub fn get_token() -> Result<(String, String), Error> {
        let data = load().unwrap_or_default();
        Ok((data.token.unwrap_or_default(), data.username))
    }

    pub fn store_pass(username: String, password: String, server: String, identity: String) -> Result<(), Error> {
        let mut data = load().unwrap_or_default();
        data.username = username;
        data.password = Some(password);
        data.server = server;
        data.identity = identity;
        store(&data)?;
        Ok(())
    }

    pub fn get_pass() -> Result<(String, String, String, String), Error> {
        let data = load().unwrap_or_default();
        Ok((data.username, data.password.unwrap_or_default(), data.server, data.identity))
    }
}
