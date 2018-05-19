extern crate serde_json;
use std::thread;
use util::json_q;

use globals;
//use std::thread;
use error::Error;

use backend::types::Backend;
use backend::types::BKResponse;
use types::StickerGroup;
use self::serde_json::Value as JsonValue;


/// Queries scalar.vector.im to list all the stickers
pub fn list(bk: &Backend) -> Result<(), Error> {
    let widget = bk.data.lock().unwrap().sticker_widget.clone();
    let widget_id = match widget {
        None => {
            let id = get_sticker_widget_id(bk)?;
            bk.data.lock().unwrap().sticker_widget = Some(id.clone());
            id
        },
        Some(id) => id.clone(),
    };

    let data = vec![
        ("widget_type", "m.stickerpicker".to_string()),
        ("widget_id", widget_id),
        ("filter_unpurchased", "true".to_string()),
    ];
    let url = bk.vurl("widgets/assets", data)?;

    let tx = bk.tx.clone();
    get!(&url,
        |r: JsonValue| {
            let mut stickers = vec![];
            for sticker_group in r["assets"].as_array().unwrap_or(&vec![]).iter() {
                let group = StickerGroup::from_json(sticker_group);
                stickers.push(group);
            }
            tx.send(BKResponse::Stickers(stickers)).unwrap();
        },
        |err| { tx.send(BKResponse::StickersError(err)).unwrap() }
    );

    Ok(())
}

pub fn get_sticker_widget_id(bk: &Backend) -> Result<String, Error> {
    let data = json!({
        "data": {},
        "type": "m.stickerpicker",
    });
    let url = bk.vurl("widgets/request", vec![])?;

    match json_q("post", &url, &data, globals::TIMEOUT) {
        Ok(r) => {
            let mut id = "".to_string();
            if let Some(i) = r["id"].as_str() {
                id = i.to_string();
            }
            if let Some(i) = r["data"]["id"].as_str() {
                id = i.to_string();
            }

            match id.is_empty() {
                true => Err(Error::BackendError),
                false => Ok(id),
            }
        },
        Err(Error::MatrixError(js)) => {
            match js["data"]["id"].as_str() {
                Some(id) => Ok(id.to_string()),
                None => Err(Error::MatrixError(js.clone())),
            }
        },
        Err(err) => { Err(err) }
    }
}
