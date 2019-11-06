use url::{Host, Url};

use crate::globals;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use std::thread;

use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;

use crate::r0::directory::post_public_rooms::request as post_public_rooms;
use crate::r0::directory::post_public_rooms::Body as PublicRoomsBody;
use crate::r0::directory::post_public_rooms::Filter as PublicRoomsFilter;
use crate::r0::directory::post_public_rooms::Parameters as PublicRoomsParameters;
use crate::r0::directory::post_public_rooms::Response as PublicRoomsResponse;
use crate::r0::directory::post_public_rooms::ThirdPartyNetworks;
use crate::r0::thirdparty::get_supported_protocols::request as get_supported_protocols;
use crate::r0::thirdparty::get_supported_protocols::Parameters as SupportedProtocolsParameters;
use crate::r0::thirdparty::get_supported_protocols::ProtocolInstance;
use crate::r0::thirdparty::get_supported_protocols::Response as SupportedProtocolsResponse;
use crate::r0::AccessToken;
use crate::types::Room;

pub fn protocols(base: Url, access_token: AccessToken) -> Result<Vec<ProtocolInstance>, Error> {
    let params = SupportedProtocolsParameters { access_token };

    get_supported_protocols(base, &params)
        .map_err(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<SupportedProtocolsResponse>()
                .map_err(Into::into)
        })
        .map(|response| {
            response
                .into_iter()
                .flat_map(|(_, protocol)| protocol.instances.into_iter())
                .collect()
        })
}

pub fn room_search(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    homeserver: Option<String>, // TODO: Use HostAndPort?
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
            })
            .map(|response| {
                data.lock().unwrap().rooms_since = response.next_batch.unwrap_or_default();

                response
                    .chunk
                    .into_iter()
                    .map(Into::into)
                    .inspect(|r: &Room| {
                        if let Some(avatar) = r.avatar.clone() {
                            if let Ok(dest) = cache_dir_path(None, &r.id) {
                                let _ =
                                    dw_media(&base, &avatar, ContentType::Download, Some(&dest));
                            }
                        }
                    })
                    .collect()
            });

        tx.send(BKResponse::DirectorySearch(query))
            .expect_log("Connection closed");
    });

    Ok(())
}
