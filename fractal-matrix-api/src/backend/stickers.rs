extern crate serde_json;
extern crate md5;
extern crate chrono;

use self::chrono::prelude::*;

use std::thread;
use util::json_q;

use globals;
//use std::thread;
use error::Error;

use backend::types::Backend;
use backend::types::BKResponse;
use backend::types::BKCommand;
use types::StickerGroup;
use types::Sticker;
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

pub fn send(bk: &Backend, roomid: String, sticker: &Sticker) -> Result<(), Error> {
    let now = Local::now();
    let msg = format!("{}{}{}", roomid, sticker.name, now.to_string());
    let digest = md5::compute(msg.as_bytes());
    // TODO: we need to generate the msg.id in the frontend
    let id = format!("{:x}", digest);

    let url = bk.url(&format!("rooms/{}/send/m.sticker/{}", roomid, id), vec![])?;

    let attrs = json!({
        "body": sticker.body.clone(),
        "url": sticker.url.clone(),
        "info": {
            "w": sticker.size.0,
            "h": sticker.size.1,
            "thumbnail_url": sticker.thumbnail.clone(),
        },
    });

    let tx = bk.tx.clone();
    query!("put", &url, &attrs,
        move |js: JsonValue| {
            let evid = js["event_id"].as_str().unwrap_or_default();
            tx.send(BKResponse::SentMsg(id, evid.to_string())).unwrap();
        },
        |_| { tx.send(BKResponse::SendMsgError(Error::SendMsgError(id))).unwrap(); }
    );

    Ok(())
}

pub fn purchase(bk: &Backend, group: &StickerGroup) -> Result<(), Error> {
    let widget = bk.data.lock().unwrap().sticker_widget.clone();
    let widget_id = match widget {
        None => {
            let id = get_sticker_widget_id(bk)?;
            bk.data.lock().unwrap().sticker_widget = Some(id.clone());
            id
        },
        Some(id) => id.clone(),
    };

    let asset = group.asset.clone();
    let data = vec![
        ("asset_type", asset.clone()),
        ("widget_id", widget_id.clone()),
        ("widget_type", "m.stickerpicker".to_string()),
    ];
    let url = bk.vurl("widgets/purchase_asset", data)?;
    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    get!(&url,
        |_| {
            if let Some(t) = itx {
                t.send(BKCommand::ListStickers).unwrap();
            }
        },
        |err| { tx.send(BKResponse::StickersError(err)).unwrap() }
    );

    Ok(())
}
