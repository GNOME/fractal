use matrix_sdk::ruma::identifiers::{DeviceIdBox, UserId};
use secret_service::EncryptionType;
use secret_service::SecretService;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use url::Url;

pub struct StoredSession {
    pub homeserver: Url,
    pub path: PathBuf,
    pub passphrase: String,
    pub user_id: UserId,
    pub access_token: String,
    pub device_id: DeviceIdBox,
}

/// Retrieves all sessions stored to the `SecretService`
pub fn restore_sessions() -> Result<Vec<StoredSession>, secret_service::Error> {
    let ss = SecretService::new(EncryptionType::Dh)?;
    let collection = ss.get_default_collection()?;

    // Sessions that contain or produce errors are ignored.
    // TODO: Return error for corrupt sessions

    let res = collection
        .get_all_items()?
        .iter()
        .fold(HashMap::new(), |mut acc, item| {
            let finder = move || -> Option<((String, String, String), (String, Option<String>))> {
                let attr = item.get_attributes().ok()?;

                let homeserver = attr.get("homeserver")?.to_string();
                let user_id = attr.get("user-id")?.to_string();
                let device_id = attr.get("device-id")?.to_string();
                let secret = String::from_utf8(item.get_secret().ok()?).ok()?;
                let path = attr.get("path").map(|s| s.to_string());
                Some(((homeserver, user_id, device_id), (secret, path)))
            };

            if let Some((key, value)) = finder() {
                acc.entry(key).or_insert(vec![]).push(value);
            }

            acc
        })
        .into_iter()
        .filter_map(|((homeserver, user_id, device_id), mut items)| {
            if items.len() != 2 {
                return None;
            }
            let (access_token, passphrase, path) = match items.pop()? {
                (secret, Some(path)) => (items.pop()?.0, secret, PathBuf::from(path)),
                (secret, None) => {
                    let item = items.pop()?;
                    (secret, item.0, PathBuf::from(item.1?))
                }
            };

            let homeserver = Url::parse(&homeserver).ok()?;
            let user_id = UserId::try_from(user_id).ok()?;
            let device_id = DeviceIdBox::try_from(device_id).ok()?;

            Some(StoredSession {
                homeserver,
                path,
                passphrase,
                user_id,
                access_token,
                device_id,
            })
        })
        .collect();

    Ok(res)
}

/// Writes a session to the `SecretService`, overwriting any previously stored session with the
/// same `homeserver`, `username` and `device-id`.
pub fn store_session(session: StoredSession) -> Result<(), secret_service::Error> {
    let ss = SecretService::new(EncryptionType::Dh)?;
    let collection = ss.get_default_collection()?;

    // Store the information for the login
    let attributes: HashMap<&str, &str> = [
        ("user-id", session.user_id.as_str()),
        ("homeserver", session.homeserver.as_str()),
        ("device-id", session.device_id.as_str()),
    ]
    .iter()
    .cloned()
    .collect();

    collection.create_item(
        "Fractal",
        attributes,
        session.access_token.as_bytes(),
        true,
        "text/plain",
    )?;

    // Store the information for the crypto store
    let attributes: HashMap<&str, &str> = [
        ("path", session.path.to_str().unwrap()),
        ("user-id", session.user_id.as_str()),
        ("homeserver", session.homeserver.as_str()),
        ("device-id", session.device_id.as_str()),
    ]
    .iter()
    .cloned()
    .collect();

    collection.create_item(
        "Fractal (Encrypted local database)",
        attributes,
        session.passphrase.as_bytes(),
        true,
        "text/plain",
    )?;

    Ok(())
}
