use url::{Host, Url};

use crate::globals;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use std::thread;

use crate::util::cache_dir_path;
use crate::util::media;
use crate::util::HTTP_CLIENT;

use crate::r0::directory::post_public_rooms::request as post_public_rooms;
use crate::r0::directory::post_public_rooms::Body as PublicRoomsBody;
use crate::r0::directory::post_public_rooms::Filter as PublicRoomsFilter;
use crate::r0::directory::post_public_rooms::Parameters as PublicRoomsParameters;
use crate::r0::directory::post_public_rooms::Response as PublicRoomsResponse;
use crate::r0::directory::post_public_rooms::ThirdPartyNetworks;
use crate::r0::thirdparty::get_supported_protocols::request as get_supported_protocols;
use crate::r0::thirdparty::get_supported_protocols::Parameters as SupportedProtocolsParameters;
use crate::r0::thirdparty::get_supported_protocols::Response as SupportedProtocolsResponse;
use crate::types::Room;

pub fn protocols(bk: &Backend) {
    let tx = bk.tx.clone();
    let access_token = bk.data.lock().unwrap().access_token.clone();

    let base = bk.get_base_url();
    let params = SupportedProtocolsParameters { access_token };

    thread::spawn(move || {
        let query = get_supported_protocols(base, &params)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<SupportedProtocolsResponse>()
                    .map_err(Into::into)
            });

        match query {
            Ok(response) => {
                let protocols = response
                    .into_iter()
                    .flat_map(|(_, protocol)| protocol.instances.into_iter())
                    .collect();

                let _ = tx.send(BKResponse::DirectoryProtocols(protocols));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::DirectoryError(err));
            }
        }
    });
}

pub fn room_search(
    bk: &Backend,
    homeserver: Option<String>,
    generic_search_term: Option<String>,
    third_party: Option<String>,
    more: bool,
) -> Result<(), Error> {
    let tx = bk.tx.clone();
    let data = bk.data.clone();

    // TODO: use transpose() when it is stabilized
    let server = homeserver
        .map(|hs| {
            Url::parse(&hs)
                .ok()
                .as_ref()
                .and_then(Url::host)
                .as_ref()
                .map(Host::to_owned)
                .map(Ok)
                .unwrap_or(Host::parse(&hs))
                .map(Some)
        })
        .unwrap_or(Ok(None))?;

    let base = bk.get_base_url();
    let access_token = data.lock().unwrap().access_token.clone();

    let since = if more {
        Some(data.lock().unwrap().rooms_since.clone())
    } else {
        None
    };

    let params = PublicRoomsParameters {
        access_token,
        server,
    };

    let body = PublicRoomsBody {
        limit: Some(globals::ROOM_DIRECTORY_LIMIT),
        filter: Some(PublicRoomsFilter {
            generic_search_term,
        }),
        since,
        third_party_networks: third_party
            .map(ThirdPartyNetworks::Only)
            .unwrap_or_default(),
    };

    thread::spawn(move || {
        let query = post_public_rooms(base.clone(), &params, &body)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)?
                    .json::<PublicRoomsResponse>()
                    .map_err(Into::into)
            });

        match query {
            Ok(response) => {
                data.lock().unwrap().rooms_since = response.next_batch.unwrap_or_default();

                let rooms = response
                    .chunk
                    .into_iter()
                    .map(Into::into)
                    .inspect(|r: &Room| {
                        if let Some(avatar) = r.avatar.clone() {
                            if let Ok(dest) = cache_dir_path("", &r.id) {
                                let _ = media(&base, &avatar, Some(&dest));
                            }
                        }
                    })
                    .collect();

                let _ = tx.send(BKResponse::DirectorySearch(rooms));
            }
            Err(err) => {
                let _ = tx.send(BKResponse::DirectoryError(err));
            }
        }
    });

    Ok(())
}
