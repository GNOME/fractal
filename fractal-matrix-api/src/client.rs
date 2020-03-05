use crate::error::Error;
use crate::globals;

use gio;
use gio::prelude::*;
use reqwest;

use std::sync::Mutex;
use std::time::Duration;

// Special URI used by gio to indicate no proxy
const PROXY_DIRECT_URI: &str = "direct://";

#[derive(Debug, Eq, PartialEq)]
pub struct ProxySettings {
    http_proxy: Vec<String>,
    https_proxy: Vec<String>,
}

impl ProxySettings {
    fn new(http_proxy: Vec<String>, https_proxy: Vec<String>) -> ProxySettings {
        ProxySettings {
            http_proxy,
            https_proxy,
        }
    }

    fn direct() -> ProxySettings {
        Self::new(
            vec![PROXY_DIRECT_URI.to_string()],
            vec![PROXY_DIRECT_URI.to_string()],
        )
    }

    pub fn current() -> Result<ProxySettings, Error> {
        Ok(Self::new(
            PROXY_RESOLVER
                .with(|resolver| resolver.lookup("http://", gio::NONE_CANCELLABLE))?
                .iter()
                .map(ToString::to_string)
                .collect(),
            PROXY_RESOLVER
                .with(|resolver| resolver.lookup("https://", gio::NONE_CANCELLABLE))?
                .iter()
                .map(ToString::to_string)
                .collect(),
        ))
    }

    pub fn apply_to_client_builder(
        &self,
        mut builder: reqwest::blocking::ClientBuilder,
    ) -> Result<reqwest::blocking::ClientBuilder, reqwest::Error> {
        // Reqwest only supports one proxy for each type

        if !self.http_proxy.is_empty() && self.http_proxy[0] != PROXY_DIRECT_URI {
            let proxy = reqwest::Proxy::http(&self.http_proxy[0])?;
            builder = builder.proxy(proxy);
        }
        if !self.https_proxy.is_empty() && self.https_proxy[0] != PROXY_DIRECT_URI {
            let proxy = reqwest::Proxy::https(&self.https_proxy[0])?;
            builder = builder.proxy(proxy);
        }

        Ok(builder)
    }
}

// gio::ProxyResolver can't be sent or shared
thread_local! {
    static PROXY_RESOLVER: gio::ProxyResolver =
        gio::ProxyResolver::get_default().expect("Couldn't get proxy resolver");
}

#[derive(Debug)]
struct ClientInner {
    client: reqwest::blocking::Client,
    proxy_settings: ProxySettings,
}

#[derive(Debug)]
pub struct Client {
    inner: Mutex<ClientInner>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            inner: Mutex::new(ClientInner {
                client: Self::build(reqwest::blocking::Client::builder()),
                proxy_settings: ProxySettings::direct(),
            }),
        }
    }

    pub fn get_client(&self) -> Result<reqwest::blocking::Client, Error> {
        // Lock first so we don't overwrite proxy settings with outdated information
        let mut inner = self.inner.lock().unwrap();

        let new_proxy_settings = ProxySettings::current()?;

        if inner.proxy_settings == new_proxy_settings {
            Ok(inner.client.clone())
        } else {
            let mut builder = reqwest::blocking::Client::builder();
            builder = new_proxy_settings.apply_to_client_builder(builder)?;
            let client = Self::build(builder);

            inner.client = client;
            inner.proxy_settings = new_proxy_settings;

            Ok(inner.client.clone())
        }
    }

    fn build(builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::Client {
        builder
            .gzip(true)
            .timeout(Duration::from_secs(globals::TIMEOUT))
            .build()
            .expect("Couldn't create a http client")
    }
}
