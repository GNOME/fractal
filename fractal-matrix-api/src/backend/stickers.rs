use chrono::prelude::*;
use serde_json::json;

use crate::backend::BackendData;
use crate::util::json_q;
use crate::util::ResultExpectLog;
use crate::util::{client_url, scalar_url};
use std::sync::{Arc, Mutex};
use std::thread;
use url::Url;

use crate::error::Error;

use crate::backend::types::BKCommand;
use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::r0::AccessToken;
use crate::types::Sticker;
use crate::types::StickerGroup;
use serde_json::Value as JsonValue;

/// Queries scalar.vector.im to list all the stickers
pub fn list(bk: &Backend, access_token: AccessToken) -> Result<(), Error> {
    let widget = bk.data.lock().unwrap().sticker_widget.clone();
    if widget.is_none() {
        get_sticker_widget_id(
            bk,
            access_token.clone(),
            BKCommand::ListStickers(access_token),
        )?;
        return Ok(());
    }

    let widget_id = widget.unwrap();
    let data = vec![
        ("widget_type", "m.stickerpicker".to_string()),
        ("widget_id", widget_id),
        ("filter_unpurchased", "true".to_string()),
    ];
    let url = vurl(&bk.data, &access_token, "widgets/assets", data)?;

    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let mut stickers = vec![];
            for sticker_group in r["assets"].as_array().unwrap_or(&vec![]).iter() {
                let group = StickerGroup::from_json(sticker_group);
                stickers.push(group);
            }
            tx.send(BKResponse::Stickers(Ok(stickers)))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::Stickers(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn get_sticker_widget_id(
    bk: &Backend,
    access_token: AccessToken,
    then: BKCommand,
) -> Result<(), Error> {
    let data = json!({
        "data": {},
        "type": "m.stickerpicker",
    });
    let d = bk.data.clone();
    let itx = bk.internal_tx.clone();

    let url = vurl(&d, &access_token, "widgets/request", vec![]).unwrap();
    post!(
        &url,
        &data,
        |r: JsonValue| {
            let mut id = String::new();
            if let Some(i) = r["id"].as_str() {
                id = i.to_string();
            }
            if let Some(i) = r["data"]["id"].as_str() {
                id = i.to_string();
            }

            let widget_id = if id.is_empty() { None } else { Some(id) };
            d.lock().unwrap().sticker_widget = widget_id;

            if let Some(t) = itx {
                t.send(then).expect_log("Connection closed");
            }
        },
        |err| {
            match err {
                Error::MatrixError(js) => {
                    let widget_id = js["data"]["id"].as_str().map(|id| id.to_string());
                    d.lock().unwrap().sticker_widget = widget_id;
                }
                _ => {
                    d.lock().unwrap().sticker_widget = None;
                }
            }

            if let Some(t) = itx {
                t.send(then).expect_log("Connection closed");
            }
        }
    );

    Ok(())
}

pub fn send(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    roomid: String,
    sticker: Sticker,
) -> Result<(), Error> {
    let now = Local::now();
    let msg = format!("{}{}{}", roomid, sticker.name, now.to_string());
    let digest = md5::compute(msg.as_bytes());
    // TODO: we need to generate the msg.id in the frontend
    let id = format!("{:x}", digest);

    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/send/m.sticker/{}", roomid, id),
        vec![],
    )?;

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
    query!(
        "put",
        &url,
        &attrs,
        move |js: JsonValue| {
            let evid = js["event_id"].as_str().unwrap_or_default();
            tx.send(BKResponse::SentMsg(Ok((id, evid.to_string()))))
                .expect_log("Connection closed");
        },
        |_| {
            tx.send(BKResponse::SentMsg(Err(Error::SendMsgError(id))))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn purchase(bk: &Backend, access_token: AccessToken, group: StickerGroup) -> Result<(), Error> {
    let widget = bk.data.lock().unwrap().sticker_widget.clone();
    if widget.is_none() {
        get_sticker_widget_id(
            bk,
            access_token.clone(),
            BKCommand::PurchaseSticker(access_token, group.clone()),
        )?;
        return Ok(());
    }

    let widget_id = widget.unwrap();
    let asset = group.asset.clone();
    let data = vec![
        ("asset_type", asset.clone()),
        ("widget_id", widget_id.clone()),
        ("widget_type", "m.stickerpicker".to_string()),
    ];
    let url = vurl(&bk.data, &access_token, "widgets/purchase_asset", data)?;
    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    get!(
        &url,
        |_| if let Some(t) = itx {
            t.send(BKCommand::ListStickers(access_token))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::Stickers(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

fn get_scalar_token(
    data: &Arc<Mutex<BackendData>>,
    access_token: &AccessToken,
) -> Result<String, Error> {
    let base = data.lock().unwrap().scalar_url.clone();
    let uid = data.lock().unwrap().user_id.clone();

    let params = &[("access_token", access_token.to_string())];
    let path = &format!("user/{}/openid/request_token", uid);
    let url = client_url(&base, path, params)?;
    let js = json_q("post", &url, &json!({}))?;

    let vurl = base
        .join("/api/register")
        .expect("Wrong URL in get_scalar_token()");
    let js = json_q("post", &vurl, &js)?;

    match js["scalar_token"].as_str() {
        Some(st) => {
            data.lock().unwrap().scalar_token = Some(st.to_string());
            Ok(st.to_string())
        }
        None => Err(Error::BackendError),
    }
}

fn vurl(
    data: &Arc<Mutex<BackendData>>,
    access_token: &AccessToken,
    path: &str,
    mut params: Vec<(&str, String)>,
) -> Result<Url, Error> {
    let base = data.lock().unwrap().scalar_url.clone();
    let tk = data
        .lock()
        .unwrap()
        .scalar_token
        .clone()
        .unwrap_or(get_scalar_token(data, access_token)?);

    params.push(("scalar_token", tk));

    scalar_url(&base, path, &params)
}
