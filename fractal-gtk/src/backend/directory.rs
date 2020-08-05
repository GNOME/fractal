use fractal_api::reqwest::Error as ReqwestError;
use fractal_api::url::{Host, ParseError as UrlError, Url};
use std::convert::TryInto;

use crate::globals;

use crate::backend::HTTP_CLIENT;
use crate::util::cache_dir_path;

use crate::types::Room;
use fractal_api::r0::directory::post_public_rooms::request as post_public_rooms;
use fractal_api::r0::directory::post_public_rooms::Body as PublicRoomsBody;
use fractal_api::r0::directory::post_public_rooms::Filter as PublicRoomsFilter;
use fractal_api::r0::directory::post_public_rooms::Parameters as PublicRoomsParameters;
use fractal_api::r0::directory::post_public_rooms::Response as PublicRoomsResponse;
use fractal_api::r0::directory::post_public_rooms::ThirdPartyNetworks;
use fractal_api::r0::thirdparty::get_supported_protocols::request as get_supported_protocols;
use fractal_api::r0::thirdparty::get_supported_protocols::Parameters as SupportedProtocolsParameters;
use fractal_api::r0::thirdparty::get_supported_protocols::ProtocolInstance;
use fractal_api::r0::thirdparty::get_supported_protocols::Response as SupportedProtocolsResponse;
use fractal_api::r0::AccessToken;

use super::{dw_media, ContentType, HandleError};
use crate::app::App;
use crate::i18n::i18n;
use crate::APPOP;

#[derive(Debug)]
pub struct DirectoryProtocolsError;

impl From<ReqwestError> for DirectoryProtocolsError {
    fn from(_: ReqwestError) -> Self {
        Self
    }
}

impl HandleError for DirectoryProtocolsError {
    fn handle_error(&self) {
        let error = i18n("Error searching for rooms");
        APPOP!(reset_directory_state);
        APPOP!(show_error, (error));
    }
}

pub fn protocols(
    base: Url,
    access_token: AccessToken,
) -> Result<Vec<ProtocolInstance>, DirectoryProtocolsError> {
    let params = SupportedProtocolsParameters { access_token };
    let request = get_supported_protocols(base, &params)?;
    let response: SupportedProtocolsResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    Ok(response
        .into_iter()
        .flat_map(|(_, protocol)| protocol.instances.into_iter())
        .collect())
}

#[derive(Debug)]
pub enum DirectorySearchError {
    InvalidHomeserverUrl(UrlError),
    Reqwest(ReqwestError),
    ParseUrl(UrlError),
}

impl From<ReqwestError> for DirectorySearchError {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}

impl From<UrlError> for DirectorySearchError {
    fn from(err: UrlError) -> Self {
        Self::ParseUrl(err)
    }
}

impl HandleError for DirectorySearchError {
    fn handle_error(&self) {
        let error = i18n("Error searching for rooms");
        APPOP!(reset_directory_state);
        APPOP!(show_error, (error));
    }
}

pub fn room_search(
    base: Url,
    access_token: AccessToken,
    homeserver: String, // TODO: Option<Use HostAndPort>?
    generic_search_term: String,
    third_party: String,
    rooms_since: Option<String>,
) -> Result<(Vec<Room>, Option<String>), DirectorySearchError> {
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
                .unwrap_or_else(|| Host::parse(&hs))
                // Remove the url::Host enum, we only need the domain string
                .map(|host| host.to_string())
                .map(Some)
        })
        .unwrap_or(Ok(None))
        .map_err(DirectorySearchError::InvalidHomeserverUrl)?;

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
    let response: PublicRoomsResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    let since = response.next_batch;
    let rooms = response
        .chunk
        .into_iter()
        .map(TryInto::try_into)
        .inspect(|r: &Result<Room, _>| {
            if let Ok(room) = r {
                if let Some(avatar) = room.avatar.clone() {
                    if let Ok(dest) = cache_dir_path(None, &room.id.to_string()) {
                        let _ = dw_media(base.clone(), &avatar, ContentType::Download, Some(dest));
                    }
                }
            }
        })
        .collect::<Result<_, UrlError>>()?;

    Ok((rooms, since))
}
