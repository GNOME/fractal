use chrono::prelude::*;
use serde_json::json;

use crate::util::json_q;
use crate::util::ResultExpectLog;
use crate::util::{client_url, scalar_url};
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
pub fn list(
    bk: &Backend,
    access_token: AccessToken,
    uid: String,
    scalar_url: Url,
    scalar_token: Option<String>,
) -> Result<(), Error> {
    if let Some(widget_id) = bk.data.lock().unwrap().sticker_widget.clone() {
        let data = vec![
            ("widget_type", "m.stickerpicker".to_string()),
            ("widget_id", widget_id),
            ("filter_unpurchased", "true".to_string()),
        ];
        let url = vurl(
            scalar_url,
            scalar_token,
            &access_token,
            uid,
            "widgets/assets",
            data,
        )?;

        let tx = bk.tx.clone();
        get!(
            url,
            |r: JsonValue| {
                let stickers = r["assets"]
                    .as_array()
                    .iter()
                    .flat_map(|arr| arr.iter())
                    .map(StickerGroup::from_json)
                    .collect();

                tx.send(BKResponse::Stickers(Ok(stickers)))
                    .expect_log("Connection closed");
            },
            |err| {
                tx.send(BKResponse::Stickers(Err(err)))
                    .expect_log("Connection closed");
            }
        );
    } else {
        get_sticker_widget_id(
            bk,
            access_token.clone(),
            uid.clone(),
            BKCommand::ListStickers(access_token, uid, scalar_url.clone(), scalar_token.clone()),
            scalar_url,
            scalar_token,
        )?;
    }

    Ok(())
}

pub fn get_sticker_widget_id(
    bk: &Backend,
    access_token: AccessToken,
    uid: String,
    then: BKCommand,
    scalar_url: Url,
    scalar_token: Option<String>,
) -> Result<(), Error> {
    let data = json!({
        "data": {},
        "type": "m.stickerpicker",
    });
    let d = bk.data.clone();
    let itx = bk.internal_tx.clone();

    let url = vurl(
        scalar_url,
        scalar_token,
        &access_token,
        uid,
        "widgets/request",
        vec![],
    )
    .unwrap();
    post!(
        url,
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
        url,
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

pub fn purchase(
    bk: &Backend,
    access_token: AccessToken,
    uid: String,
    group: StickerGroup,
    scalar_url: Url,
    scalar_token: Option<String>,
) -> Result<(), Error> {
    if let Some(widget_id) = bk.data.lock().unwrap().sticker_widget.clone() {
        let asset = group.asset.clone();
        let data = vec![
            ("asset_type", asset.clone()),
            ("widget_id", widget_id.clone()),
            ("widget_type", "m.stickerpicker".to_string()),
        ];
        let url = vurl(
            scalar_url.clone(),
            scalar_token.clone(),
            &access_token,
            uid.clone(),
            "widgets/purchase_asset",
            data,
        )?;
        let tx = bk.tx.clone();
        let itx = bk.internal_tx.clone();
        get!(
            url,
            |_| if let Some(t) = itx {
                t.send(BKCommand::ListStickers(
                    access_token,
                    uid,
                    scalar_url,
                    scalar_token,
                ))
                .expect_log("Connection closed");
            },
            |err| {
                tx.send(BKResponse::Stickers(Err(err)))
                    .expect_log("Connection closed");
            }
        );

        Ok(())
    } else {
        get_sticker_widget_id(
            bk,
            access_token.clone(),
            uid.clone(),
            BKCommand::PurchaseSticker(
                access_token,
                uid,
                group.clone(),
                scalar_url.clone(),
                scalar_token.clone(),
            ),
            scalar_url,
            scalar_token,
        )?;

        Ok(())
    }
}

fn get_scalar_token(
    scalar_url: Url,
    access_token: &AccessToken,
    uid: String,
) -> Result<String, Error> {
    let params = &[("access_token", access_token.to_string())];
    let path = &format!("user/{}/openid/request_token", uid);
    let url = client_url(&scalar_url, path, params)?;
    let js = json_q("post", url, &json!({}))?;

    let vurl = scalar_url
        .join("/api/register")
        .expect("Wrong URL in get_scalar_token()");

    json_q("post", vurl, &js)?["scalar_token"]
        .as_str()
        .map(Into::into)
        .ok_or(Error::BackendError)
}

fn vurl(
    s_url: Url,
    scalar_token: Option<String>,
    access_token: &AccessToken,
    uid: String,
    path: &str,
    mut params: Vec<(&str, String)>,
) -> Result<Url, Error> {
    let tk = scalar_token.unwrap_or(get_scalar_token(s_url.clone(), access_token, uid)?);

    params.push(("scalar_token", tk));

    scalar_url(&s_url, path, &params)
}
