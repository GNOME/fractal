use url::{Host, Url};

use crate::globals;

use crate::error::Error;

use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::ContentType;
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
    let request = get_supported_protocols(base, &params)?;
    let response: SupportedProtocolsResponse =
        HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok(response
        .into_iter()
        .flat_map(|(_, protocol)| protocol.instances.into_iter())
        .collect())
}

pub fn room_search(
    base: Url,
    access_token: AccessToken,
    homeserver: String, // TODO: Option<Use HostAndPort>?
    generic_search_term: String,
    third_party: String,
    rooms_since: Option<String>,
) -> Result<(Vec<Room>, Option<String>), Error> {
    let homeserver = Some(homeserver).filter(|hs| !hs.is_empty());
    let generic_search_term = Some(generic_search_term).filter(|q| !q.is_empty());
    let third_party = Some(third_party).filter(|tp| !tp.is_empty());

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

    let params = PublicRoomsParameters {
        access_token,
        server,
    };

    let body = PublicRoomsBody {
        limit: Some(globals::ROOM_DIRECTORY_LIMIT),
        filter: Some(PublicRoomsFilter {
            generic_search_term,
        }),
        since: rooms_since,
        third_party_networks: third_party
            .map(ThirdPartyNetworks::Only)
            .unwrap_or_default(),
    };

    let request = post_public_rooms(base.clone(), &params, &body)?;
    let response: PublicRoomsResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let since = response.next_batch;
    let rooms = response
        .chunk
        .into_iter()
        .map(Into::into)
        .inspect(|r: &Room| {
            if let Some(avatar) = r.avatar.clone() {
                if let Ok(dest) = cache_dir_path(None, &r.id.to_string()) {
                    let _ = dw_media(base.clone(), &avatar, ContentType::Download, Some(dest));
                }
            }
        })
        .collect();

    Ok((rooms, since))
}
