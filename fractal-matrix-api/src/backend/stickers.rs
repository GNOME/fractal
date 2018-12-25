use chrono::prelude::*;
use md5;
use JsonValue;

use std::{
    sync::{Arc, Mutex},
    thread,
};
use url::Url;
use util::{client_url, json_q, scalar_url};

use error::Error;
use globals;

pub use backend::types::{BKResponse, Backend};
use backend::{types::BKCommand, BackendData};

use types::{Sticker, StickerGroup};

impl Backend {
    /// Queries scalar.vector.im to list all the stickers
    pub fn list_stickers(&self) {
        let ctx = self.tx.clone();
        let widget = self.data.lock().unwrap().sticker_widget.clone();
        if widget.is_none() {
            self.get_sticker_widget_id(BKCommand::ListStickers);
            return;
        }

        let data = vec![
            ("widget_type", "m.stickerpicker".to_string()),
            ("widget_id", widget.unwrap()),
            ("filter_unpurchased", "true".to_string()),
        ];
        let url = vurl(&self.data, "widgets/assets", data).unwrap();

        get!(
            &url,
            |r: JsonValue| {
                let stickers = r["assets"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|sticker_group| StickerGroup::from_json(sticker_group))
                    .collect();
                ctx.send(BKResponse::Stickers(stickers)).unwrap();
            },
            |err| ctx.send(BKResponse::StickersError(err)).unwrap()
        );
    }

    pub fn send_sticker(&self, room_id: String, sticker: Sticker) {
        let ctx = self.tx.clone();
        let msg = format!("{}{}{}", &room_id, sticker.name, Local::now().to_string());
        let digest = md5::compute(msg.as_bytes());
        // TODO: we need to generate the msg.id in the frontend
        let id = format!("{:x}", digest);

        let url = self.url(&format!("rooms/{}/send/m.sticker/{}", &room_id, id), vec![]);

        let attrs = json!({
            "body": sticker.body.clone(),
            "url": sticker.url.clone(),
            "info": {
                "w": sticker.size.0,
                "h": sticker.size.1,
                "thumbnail_url": sticker.thumbnail.clone(),
            },
        });

        put!(
            &url,
            &attrs,
            move |js: JsonValue| {
                let evid = js["event_id"].as_str().unwrap_or_default().to_string();
                ctx.send(BKResponse::SentMsg(id, evid)).unwrap();
            },
            |_| ctx
                .send(BKResponse::SendMsgError(Error::SendMsgError(id)))
                .unwrap()
        );
    }

    pub fn purchase_sticker(&self, group: StickerGroup) {
        let ctx = self.tx.clone();
        let itx = self.internal_tx.clone();
        let widget = self.data.lock().unwrap().sticker_widget.clone();
        if widget.is_none() {
            self.get_sticker_widget_id(BKCommand::PurchaseSticker(group.clone()));
            return;
        }

        let widget_id = widget.unwrap();
        let asset = group.asset.clone();
        let data = vec![
            ("asset_type", asset.clone()),
            ("widget_id", widget_id.clone()),
            ("widget_type", "m.stickerpicker".to_string()),
        ];
        let url = vurl(&self.data, "widgets/purchase_asset", data).unwrap();
        get!(
            &url,
            |_| itx
                .map(|t| t.send(BKCommand::ListStickers).unwrap())
                .unwrap_or_default(),
            |err| ctx.send(BKResponse::StickersError(err)).unwrap()
        );
    }

    fn get_sticker_widget_id(&self, then: BKCommand) {
        let ctx = self.internal_tx.clone();
        let d = self.data.clone();

        thread::spawn(move || {
            let data = json!({
                "data": {},
                "type": "m.stickerpicker",
            });
            let url = vurl(&d, "widgets/request", vec![]).unwrap();

            d.lock().unwrap().sticker_widget = json_q("post", &url, &data, globals::TIMEOUT)
                .or_else(|err| {
                    if let Error::MatrixError(js) = err {
                        Ok(js)
                    } else {
                        Err(err)
                    }
                })
                .ok()
                .and_then(|r| {
                    r["data"]["id"]
                        .as_str()
                        .or(r["id"].as_str())
                        .filter(|id| !id.is_empty())
                        .map(Into::into)
                });

            ctx.map(|ctx| ctx.send(then).unwrap());
        });
    }
}

fn get_base_url(data: &Arc<Mutex<BackendData>>) -> Result<Url, Error> {
    let s = data.lock().unwrap().server_url.clone();
    let url = Url::parse(&s)?;
    Ok(url)
}

fn url(
    data: &Arc<Mutex<BackendData>>,
    path: &str,
    mut params: Vec<(&str, String)>,
) -> Result<Url, Error> {
    let base = get_base_url(data)?;
    let tk = data.lock().unwrap().access_token.clone();

    params.push(("access_token", tk));

    client_url(&base, path, &params)
}

fn get_scalar_token(data: &Arc<Mutex<BackendData>>) -> Result<String, Error> {
    let s = data.lock().unwrap().scalar_url.clone();
    let uid = data.lock().unwrap().user_id.clone();

    let url = url(data, &format!("user/{}/openid/request_token", uid), vec![])?;
    let js = json_q("post", &url, &json!({}), globals::TIMEOUT)?;

    let vurl = Url::parse(&format!("{}/api/register", s))?;
    let js = json_q("post", &vurl, &js, globals::TIMEOUT)?;

    js["scalar_token"]
        .as_str()
        .map(|st| {
            let st = st.to_string();
            data.lock().unwrap().scalar_token = Some(st.clone());
            st
        })
        .ok_or(Error::BackendError)
}

fn vurl(
    data: &Arc<Mutex<BackendData>>,
    path: &str,
    mut params: Vec<(&str, String)>,
) -> Result<Url, Error> {
    let s = data.lock().unwrap().scalar_url.clone();
    let base = Url::parse(&s)?;
    let token = data.lock().unwrap().scalar_token.clone();
    let tk = token
        .ok_or(Error::BackendError)
        .or(get_scalar_token(data))?;

    params.push(("scalar_token", tk));

    scalar_url(&base, path, &params)
}
