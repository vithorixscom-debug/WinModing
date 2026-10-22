use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, USER_AGENT};
use tokio::io::AsyncWriteExt;

use crate::cache::MetadataCache;
use crate::domain::ContentHash;
use crate::error::LauncherError;
use crate::hashutil::verify_bytes;
use crate::protocol::{env_nonempty, force_https, USER_AGENT as APP_USER_AGENT};

const MAX_ATTEMPTS: u32 = 3;
const API_CACHE_TTL_SECS: i64 = 600;
const SEARCH_CACHE_TTL_SECS: i64 = 90;
const MANIFEST_CACHE_TTL_SECS: i64 = 43_200;
const API_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(6);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const SKIP_CONNECT_SECS: u64 = 10 * 60;
const SKIP_UNAVAILABLE_SECS: u64 = 30 * 60;
const MILLIDA_API: &str = "https://api.millida.net/v2";
const MILLIDA_MR: &str = "https://api.millida.net/v2/launcher/mr";
const MILLIDA_CF: &str = "https://api.millida.net/v2/launcher/cf";

#[derive(Clone)]
pub struct HttpPool {
    client: reqwest::Client,
    /// Extra clients: SOCKS with remote DNS, then direct. Used when the
    /// first hop (HTTP system proxy) still exits inside a blocked network.
    alts: Vec<reqwest::Client>,
}

enum ClientHop {
    /// WinINet / env proxy, same road as the browser (v2rayN, Clash, Hiddify).
    System,
    Proxy(String),
    Direct,
}

impl HttpPool {
    pub fn new() -> Result<Self, LauncherError> {
        let discovered = discover_proxy_urls();
        let mut hops = vec![ClientHop::System];
        for url in &discovered {
            hops.push(ClientHop::Proxy(url.clone()));
        }
        if !discovered.is_empty() || env_proxy_is_set() {
            hops.push(ClientHop::Direct);
        }
        let client = build_client(&hops[0])?;
        let mut alts = Vec::new();
        for hop in hops.iter().skip(1) {
            if let Ok(extra) = build_client(hop) {
                alts.push(extra);
            }
        }
        Ok(Self { client, alts })
    }

    fn clients(&self) -> impl Iterator<Item = &reqwest::Client> {
        std::iter::once(&self.client).chain(self.alts.iter())
    }

    pub async fn get_text(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, LauncherError> {
        let candidates = usable_candidates(alternate_urls(url, extra_headers));
        if candidates.len() <= 1 {
            return self
                .get_text_once(candidates.first().map(String::as_str).unwrap_or(url), extra_headers)
                .await;
        }
        self.first_catalog_success(candidates, extra_headers).await
    }

    async fn first_catalog_success(
        &self,
        candidates: Vec<String>,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, LauncherError> {
        let ordered = with_sticky_host(with_demoted_mirror(candidates));
        let has_sticky = ordered
            .first()
            .and_then(|url| {
                net_memory()
                    .lock()
                    .ok()
                    .and_then(|memory| memory.sticky.get(host_family(url)).cloned())
            })
            .is_some();
        let mut skip = 0usize;
        if has_sticky {
            if let Some(preferred) = ordered.first() {
                match self.get_text_once(preferred, extra_headers).await {
                    Ok(body) => {
                        remember_sticky_host(preferred);
                        return Ok(body);
                    }
                    Err(err) => {
                        mark_skip_from_err(preferred, &err);
                        skip = 1;
                    }
                }
            }
        }
        let headers: Vec<(String, String)> = extra_headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        let mut pending = FuturesUnordered::new();
        for candidate in ordered.into_iter().skip(skip) {
            let http = self.clone();
            let headers = headers.clone();
            pending.push(async move {
                let refs: Vec<(&str, &str)> = headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str()))
                    .collect();
                match http.get_text_once(&candidate, &refs).await {
                    Ok(body) => Ok((candidate, body)),
                    Err(err) => Err((candidate, err)),
                }
            });
        }
        let mut connect = None;
        let mut other = None;
        while let Some(item) = pending.next().await {
            match item {
                Ok((url, body)) => {
                    remember_sticky_host(&url);
                    return Ok(body);
                }
                Err((url, err)) => {
                    mark_skip_from_err(&url, &err);
                    if is_connect_like(&err) {
                        connect = Some(err);
                    } else if is_unavailable_mirror(&err) {
                        other.get_or_insert(err);
                    } else {
                        other = Some(err);
                    }
                }
            }
        }
        Err(connect.or(other).unwrap_or_else(|| {
            LauncherError::Message("catalog hosts did not answer".into())
        }))
    }

    async fn get_text_once(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, LauncherError> {
        let response = self.send(url, extra_headers).await?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(LauncherError::RateLimited {
                provider: host_of(url),
            });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LauncherError::Provider {
                provider: host_of(url),
                status: status.as_u16(),
                body: trim_body(&body),
            });
        }
        response
            .text()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<T, LauncherError> {
        let text = self.get_text(url, extra_headers).await?;
        serde_json::from_str(&text).map_err(|err| LauncherError::CorruptVersion {
            reason: format!("invalid JSON from {url}: {err}"),
        })
    }

    pub async fn cached_text(
        &self,
        cache: &MetadataCache,
        key: &str,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, LauncherError> {
        let live_search = url.contains("/search") || url.contains("/mods/search");
        if let Some(hit) = cache.cached_body(key).await? {
            if looks_like_json(&hit) {
                return Ok(hit);
            }
        }
        let body = self.get_text(url, extra_headers).await?;
        if looks_like_json(&body) {
            let ttl = if url.contains("version_manifest") {
                MANIFEST_CACHE_TTL_SECS
            } else if live_search {
                SEARCH_CACHE_TTL_SECS
            } else {
                API_CACHE_TTL_SECS
            };
            cache.store_body(key, &body, ttl).await?;
        }
        Ok(body)
    }

    pub async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<T, LauncherError> {
        let response = self
            .client
            .post(url)
            .form(&form)
            .send()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })?;
        self.read_json(url, response).await
    }

    pub async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &impl serde::Serialize,
    ) -> Result<T, LauncherError> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })?;
        self.read_json(url, response).await
    }

    pub async fn download_to(&self, url: &str, dest: &Path) -> Result<(), LauncherError> {
        let mut last = None;
        for candidate in with_demoted_mirror(usable_candidates(alternate_urls(url, &[]))) {
            match self.download_to_once(&candidate, dest).await {
                Ok(()) => return Ok(()),
                Err(err) if err.should_try_mirror() => {
                    mark_skip_from_err(&candidate, &err);
                    if is_connect_like(&err) {
                        demote_family_of(&candidate);
                    }
                    last = Some(err);
                }
                Err(err) => return Err(err),
            }
        }
        Err(last.unwrap_or_else(|| LauncherError::Message(format!("download failed: {url}"))))
    }

    async fn download_to_once(&self, url: &str, dest: &Path) -> Result<(), LauncherError> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| crate::error::io_at(parent, err))?;
        }
        let response = self.get_url(url).await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(LauncherError::Provider {
                provider: host_of(url),
                status,
                body: trim_body(&body),
            });
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })?;
        tokio::fs::write(dest, &bytes)
            .await
            .map_err(|err| crate::error::io_at(dest, err))?;
        Ok(())
    }

    /// Download `url` into `dest` only after the declared hash matches.
    /// A short read or a transport error is retried. A stable hash mismatch is not.
    pub async fn download_verified(
        &self,
        url: &str,
        dest: &Path,
        expected: &ContentHash,
    ) -> Result<(), LauncherError> {
        if expected.hex.is_empty() {
            return Err(LauncherError::MissingHash {
                url: url.to_string(),
            });
        }
        if dest.exists() && crate::hashutil::verify_file(dest, expected).is_ok() {
            return Ok(());
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| crate::error::io_at(parent, err))?;
        }
        let partial = partial_path(dest);
        let mut last_error = LauncherError::Message("download did not start".into());
        for candidate in with_demoted_mirror(usable_candidates(alternate_urls(url, &[]))) {
            for attempt in 1..=MAX_ATTEMPTS {
                match self.download_once(&candidate, &partial, expected).await {
                    Ok(()) => {
                        tokio::fs::rename(&partial, dest)
                            .await
                            .map_err(|err| crate::error::io_at(dest, err))?;
                        return Ok(());
                    }
                    Err(err) if err.is_retryable() && attempt < MAX_ATTEMPTS => {
                        let _ = tokio::fs::remove_file(&partial).await;
                        last_error = err;
                        tokio::time::sleep(Duration::from_millis(200 * u64::from(attempt))).await;
                    }
                    Err(err) if err.should_try_mirror() => {
                        mark_skip_from_err(&candidate, &err);
                        if is_connect_like(&err) {
                            demote_family_of(&candidate);
                        }
                        let _ = tokio::fs::remove_file(&partial).await;
                        last_error = err;
                        break;
                    }
                    Err(err) => {
                        let _ = tokio::fs::remove_file(&partial).await;
                        return Err(err);
                    }
                }
            }
        }
        let _ = tokio::fs::remove_file(&partial).await;
        Err(last_error)
    }

    async fn download_once(
        &self,
        url: &str,
        partial: &Path,
        expected: &ContentHash,
    ) -> Result<(), LauncherError> {
        let response = self.get_url(url).await?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(LauncherError::RateLimited {
                provider: host_of(url),
            });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LauncherError::Provider {
                provider: host_of(url),
                status: status.as_u16(),
                body: trim_body(&body),
            });
        }
        let declared = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(partial)
            .await
            .map_err(|err| crate::error::io_at(partial, err))?;
        let mut received = 0u64;
        let mut payload = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })?;
            received += chunk.len() as u64;
            file.write_all(&chunk)
                .await
                .map_err(|err| crate::error::io_at(partial, err))?;
            payload.extend_from_slice(&chunk);
        }
        file.flush()
            .await
            .map_err(|err| crate::error::io_at(partial, err))?;
        drop(file);
        if let Some(expected_len) = declared {
            if received != expected_len {
                return Err(LauncherError::NetworkInterrupted {
                    url: url.to_string(),
                    received,
                    expected: expected_len,
                });
            }
        }
        verify_bytes(&payload, expected, &partial.display().to_string())?;
        Ok(())
    }

    async fn send(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<reqwest::Response, LauncherError> {
        let hop_count = 1 + self.alts.len();
        if hop_count <= 1 {
            return dispatch_get(&self.client, url, extra_headers).await;
        }
        let headers: Vec<(String, String)> = extra_headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        let mut pending = FuturesUnordered::new();
        for index in 0..hop_count {
            let http = self.clone();
            let url = url.to_string();
            let headers = headers.clone();
            pending.push(async move {
                let client = http.clients().nth(index).expect("http hop").clone();
                let refs: Vec<(&str, &str)> = headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str()))
                    .collect();
                dispatch_get(&client, &url, &refs).await
            });
        }
        let mut last = None;
        while let Some(item) = pending.next().await {
            match item {
                Ok(response) => return Ok(response),
                Err(err) if err.should_try_mirror() => last = Some(err),
                Err(err) => return Err(err),
            }
        }
        Err(last.unwrap_or_else(|| LauncherError::Message(format!("request failed: {url}"))))
    }

    async fn get_url(&self, url: &str) -> Result<reqwest::Response, LauncherError> {
        let hop_count = 1 + self.alts.len();
        if hop_count <= 1 {
            return self
                .client
                .get(url)
                .send()
                .await
                .map_err(|source| network_err(url, source));
        }
        let mut pending = FuturesUnordered::new();
        for index in 0..hop_count {
            let http = self.clone();
            let url = url.to_string();
            pending.push(async move {
                let client = http.clients().nth(index).expect("http hop").clone();
                client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|source| network_err(&url, source))
            });
        }
        let mut last = None;
        while let Some(item) = pending.next().await {
            match item {
                Ok(response) => return Ok(response),
                Err(err) => last = Some(err),
            }
        }
        Err(last.unwrap_or_else(|| LauncherError::Message(format!("request failed: {url}"))))
    }

    async fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        response: reqwest::Response,
    ) -> Result<T, LauncherError> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LauncherError::Auth {
                reason: format!("HTTP {} from {url}: {}", status.as_u16(), trim_body(&body)),
            });
        }
        response
            .json()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })
    }
}

fn build_client(hop: &ClientHop) -> Result<reqwest::Client, LauncherError> {
    let mut builder = reqwest::Client::builder()
        .pool_max_idle_per_host(16)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(20))
        .tcp_nodelay(true)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(Duration::from_secs(120))
        .user_agent(APP_USER_AGENT)
        .gzip(true)
        .http1_only();
    match hop {
        ClientHop::System => {}
        ClientHop::Proxy(url) => {
            let proxy = reqwest::Proxy::all(url).map_err(|source| LauncherError::Network {
                url: url.clone(),
                source,
            })?;
            builder = builder.proxy(proxy);
        }
        ClientHop::Direct => {
            builder = builder.no_proxy();
        }
    }
    builder.build().map_err(|source| LauncherError::Network {
        url: "client-init".into(),
        source,
    })
}

async fn dispatch_get(
    client: &reqwest::Client,
    url: &str,
    extra_headers: &[(&str, &str)],
) -> Result<reqwest::Response, LauncherError> {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(APP_USER_AGENT) {
        headers.insert(USER_AGENT, value);
    }
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    for (name, value) in extra_headers {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(header_name, header_value);
        }
    }
    client
        .get(url)
        .headers(headers)
        .timeout(attempt_timeout(url))
        .send()
        .await
        .map_err(|source| network_err(url, source))
}

fn attempt_timeout(url: &str) -> Duration {
    if url.contains("/launcher/dl?") {
        Duration::from_secs(20)
    } else {
        API_ATTEMPT_TIMEOUT
    }
}

pub fn partial_path(dest: &Path) -> std::path::PathBuf {
    let mut name = dest
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_else(|| "download".into());
    name.push(".partial");
    dest.with_file_name(name)
}

fn network_err(url: &str, source: reqwest::Error) -> LauncherError {
    let mut detail = source.to_string();
    let mut walk = std::error::Error::source(&source);
    while let Some(inner) = walk {
        let piece = inner.to_string();
        if !detail.contains(&piece) {
            detail.push_str(" -> ");
            detail.push_str(&piece);
        }
        walk = inner.source();
    }
    LauncherError::Message(format!("network request to {url} failed: {detail}"))
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_string()))
        .unwrap_or_else(|| "remote".into())
}

fn push_unique(out: &mut Vec<String>, next: String) {
    if !out.iter().any(|item| item == &next) {
        out.push(next);
    }
}

fn looks_like_json(body: &str) -> bool {
    let trimmed = body.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

fn has_curseforge_key(extra_headers: &[(&str, &str)]) -> bool {
    extra_headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("x-api-key") && !value.trim().is_empty())
}

fn env_proxy_is_set() -> bool {
    ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
        .iter()
        .any(|name| {
            std::env::var(name)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        })
}

fn discover_proxy_urls() -> Vec<String> {
    if env_proxy_is_set() {
        return Vec::new();
    }
    let mut urls = Vec::new();
    if let Some(url) = windows_proxy_url() {
        let http = prefer_socks5h(&url);
        if let Some(sibling) = v2ray_socks_sibling(&http) {
            if proxy_listen_open(&sibling) {
                push_unique(&mut urls, sibling);
            }
        }
        push_unique(&mut urls, http);
    }
    for (url, addr) in [
        ("socks5h://127.0.0.1:10808", "127.0.0.1:10808"),
        ("http://127.0.0.1:10809", "127.0.0.1:10809"),
        ("http://127.0.0.1:7890", "127.0.0.1:7890"),
        ("socks5h://127.0.0.1:7891", "127.0.0.1:7891"),
    ] {
        if local_port_open(addr) {
            push_unique(&mut urls, url.to_string());
        }
    }
    urls
}

fn prefer_socks5h(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("socks5://") {
        format!("socks5h://{rest}")
    } else {
        url.to_string()
    }
}

fn v2ray_socks_sibling(http_proxy: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(http_proxy).ok()?;
    let host = parsed.host_str()?;
    match parsed.port() {
        Some(10809) => Some(format!("socks5h://{host}:10808")),
        Some(7890) => Some(format!("socks5h://{host}:7891")),
        _ => None,
    }
}

fn proxy_listen_open(proxy_url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(proxy_url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let Some(port) = parsed.port_or_known_default() else {
        return false;
    };
    local_port_open(&format!("{host}:{port}"))
}

fn local_port_open(addr: &str) -> bool {
    let Ok(addr) = addr.parse::<std::net::SocketAddr>() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

fn windows_proxy_url() -> Option<String> {
    #[cfg(windows)]
    {
        read_windows_proxy_url()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn read_windows_proxy_url() -> Option<String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;
    let enabled: u32 = key.get_value("ProxyEnable").ok()?;
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    parse_windows_proxy_server(&server)
}

pub fn parse_windows_proxy_server(server: &str) -> Option<String> {
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    if !server.contains('=') {
        return Some(normalize_proxy_url(server));
    }
    let mut https = None;
    let mut http = None;
    let mut socks = None;
    for part in server.split([';', ' ', '\t']) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "https" => https = Some(value.to_string()),
            "http" => http = Some(value.to_string()),
            "socks" | "socks5" | "socks4" => socks = Some(value.to_string()),
            _ => {}
        }
    }
    if let Some(value) = https.or(http) {
        return Some(normalize_proxy_url(&value));
    }
    socks.map(|value| {
        if value.contains("://") {
            prefer_socks5h(&value)
        } else {
            format!("socks5h://{value}")
        }
    })
}

fn normalize_proxy_url(value: &str) -> String {
    let value = value.trim();
    if value.contains("://") {
        value.to_string()
    } else if value.starts_with("socks") {
        format!("socks5://{value}")
    } else {
        format!("http://{value}")
    }
}

const MODRINTH_API_PREFIXES: &[&str] = &[
    "https://api.modrinth.com",
    "https://staging-api.modrinth.com",
    MILLIDA_MR,
    "https://mod.mcimirror.top/modrinth",
];

const CURSEFORGE_API_PREFIXES: &[&str] = &[
    MILLIDA_CF,
    "https://api.curse.tools/v1",
    "https://api.curseforge.com/v1",
    "https://cfproxy.fly.dev/v1",
    "https://mod.mcimirror.top/curseforge/v1",
];

const MODRINTH_CDN_PREFIXES: &[&str] = &[
    "https://cdn.modrinth.com",
    "https://cdn-alt.modrinth.com",
];

const MILLIDA_FILE_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "media.forgecdn.net",
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "files.minecraftforge.net",
    "maven.minecraftforge.net",
    "maven.neoforged.net",
    "api.adoptium.net",
    "api.azul.com",
    "cdn.azul.com",
    "launchermeta.mojang.com",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "launcher.mojang.com",
    "libraries.minecraft.net",
    "resources.download.minecraft.net",
];

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct NetMemory {
    #[serde(default)]
    sticky: HashMap<String, String>,
    #[serde(default)]
    skip_until: HashMap<String, u64>,
}

fn routes_file() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("WinModing")
        .join("net-routes.json")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn net_memory() -> &'static Mutex<NetMemory> {
    static MEMORY: OnceLock<Mutex<NetMemory>> = OnceLock::new();
    MEMORY.get_or_init(|| {
        let loaded = std::fs::read(routes_file())
            .ok()
            .and_then(|body| serde_json::from_slice(&body).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn save_net_memory(memory: &NetMemory) {
    if let Some(parent) = routes_file().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(body) = serde_json::to_vec_pretty(memory) {
        let _ = std::fs::write(routes_file(), body);
    }
}

fn demoted_families() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn is_millida_proxy_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .is_some_and(|parsed| crate::protocol::is_millida_relay(&parsed))
}

fn millida_inner_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !parsed
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("api.millida.net"))
    {
        return None;
    }
    if !parsed.path().starts_with("/v2/launcher/dl") {
        return None;
    }
    parsed
        .query_pairs()
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.into_owned())
}

fn millida_file_proxy(url: &str) -> Option<String> {
    if is_millida_proxy_url(url) {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let allowed = MILLIDA_FILE_HOSTS.iter().any(|item| *item == host)
        || (host == "github.com" && parsed.path().starts_with("/adoptium/"));
    if !allowed {
        return None;
    }
    Some(format!(
        "{MILLIDA_API}/launcher/dl?url={}",
        urlencoding::encode(url)
    ))
}

fn host_family(url: &str) -> &'static str {
    let target = millida_inner_url(url);
    let url = target.as_deref().unwrap_or(url);
    if url.contains("cdn.modrinth") || url.contains("cdn-alt.modrinth") {
        "modrinth-cdn"
    } else if url.contains("millida.net/v2/launcher/mr")
        || url.contains("api.modrinth.com")
        || url.contains("staging-api.modrinth.com")
        || url.contains("mcimirror.top/modrinth")
    {
        "modrinth-api"
    } else if url.contains("millida.net/v2/launcher/cf")
        || url.contains("curseforge.com")
        || url.contains("curse.tools")
        || url.contains("cfproxy.fly.dev")
        || url.contains("mcimirror.top/curseforge")
    {
        "curseforge-api"
    } else if url.contains("forgecdn.net") {
        "forgecdn"
    } else if url.contains("fabricmc.")
        || url.contains("quiltmc.")
        || url.contains("minecraftforge.")
        || url.contains("neoforged.")
    {
        "loader"
    } else if url.contains("mojang.com") || url.contains("minecraft.net") {
        "mojang"
    } else if url.contains("adoptium.net") || url.contains("azul.com") {
        "java"
    } else {
        "other"
    }
}

fn demote_family_of(url: &str) {
    if is_millida_proxy_url(url) {
        return;
    }
    if let Ok(mut set) = demoted_families().lock() {
        set.insert(host_family(url).to_string());
    }
}

fn catalog_uses_millida() -> bool {
    net_memory()
        .lock()
        .ok()
        .is_some_and(|memory| memory.sticky.values().any(|host| host.contains("millida.net")))
}

fn with_demoted_mirror(mut candidates: Vec<String>) -> Vec<String> {
    let Some(first) = candidates.first() else {
        return candidates;
    };
    let family = host_family(first);
    let demoted = demoted_families()
        .lock()
        .ok()
        .is_some_and(|set| set.contains(family));
    if !demoted && !catalog_uses_millida() {
        return candidates;
    }
    if let Some(index) = candidates.iter().position(|item| is_millida_proxy_url(item)) {
        let winner = candidates.remove(index);
        candidates.insert(0, winner);
    }
    candidates
}

fn remember_sticky_host(url: &str) {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            if let Ok(mut memory) = net_memory().lock() {
                memory
                    .sticky
                    .insert(host_family(url).into(), host.to_string());
                memory.skip_until.remove(host);
                save_net_memory(&memory);
            }
        }
    }
}

fn with_sticky_host(mut candidates: Vec<String>) -> Vec<String> {
    let Some(first) = candidates.first() else {
        return candidates;
    };
    let family = host_family(first);
    let preferred = net_memory()
        .lock()
        .ok()
        .and_then(|memory| memory.sticky.get(family).cloned());
    let Some(host) = preferred else {
        return candidates;
    };
    if let Some(index) = candidates.iter().position(|item| {
        reqwest::Url::parse(item)
            .ok()
            .and_then(|parsed| parsed.host_str().map(|value| value.eq_ignore_ascii_case(&host)))
            .unwrap_or(false)
    }) {
        let winner = candidates.remove(index);
        candidates.insert(0, winner);
    }
    candidates
}

fn is_skipped_host(url: &str) -> bool {
    let host = host_of(url);
    net_memory()
        .lock()
        .ok()
        .and_then(|memory| memory.skip_until.get(&host).copied())
        .is_some_and(|until| until > unix_now())
}

fn mark_skip_from_err(url: &str, err: &LauncherError) {
    let secs = if is_unavailable_mirror(err) {
        SKIP_UNAVAILABLE_SECS
    } else if is_connect_like(err) {
        SKIP_CONNECT_SECS
    } else {
        return;
    };
    let host = host_of(url);
    if host == "remote" || host == "127.0.0.1" || host == "localhost" {
        return;
    }
    if let Ok(mut memory) = net_memory().lock() {
        memory
            .skip_until
            .insert(host, unix_now().saturating_add(secs));
        save_net_memory(&memory);
    }
}

fn usable_candidates(candidates: Vec<String>) -> Vec<String> {
    let live: Vec<String> = candidates
        .iter()
        .filter(|url| !is_skipped_host(url))
        .cloned()
        .collect();
    if live.is_empty() {
        candidates
    } else {
        live
    }
}

fn expand_from_prefixes(url: &str, prefixes: &[&str], extras: &[String]) -> Option<Vec<String>> {
    let rest = prefixes.iter().find_map(|prefix| url.strip_prefix(*prefix))?;
    let mut out = Vec::new();
    for prefix in extras.iter().map(String::as_str).chain(prefixes.iter().copied()) {
        let trimmed = prefix.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        push_unique(&mut out, format!("{trimmed}{rest}"));
    }
    Some(out)
}

fn is_connect_like(err: &LauncherError) -> bool {
    match err {
        LauncherError::Network { .. } | LauncherError::NetworkInterrupted { .. } => true,
        LauncherError::Message(text) => {
            text.contains("network request")
                || text.contains("Connect")
                || text.contains("10054")
                || text.contains("10060")
        }
        _ => false,
    }
}

fn is_unavailable_mirror(err: &LauncherError) -> bool {
    matches!(
        err,
        LauncherError::Provider {
            status: 502 | 503 | 504 | 521 | 522 | 523 | 530,
            ..
        }
    )
}

fn alternate_urls(url: &str, extra_headers: &[(&str, &str)]) -> Vec<String> {
    let url = force_https(url);
    let mut extras = Vec::new();
    if let Some(custom) = env_nonempty("WINMODING_MODRINTH_API") {
        extras.push(custom.trim_end_matches('/').to_string());
    }
    if let Some(expanded) = expand_from_prefixes(&url, MODRINTH_API_PREFIXES, &extras) {
        return expanded;
    }
    extras.clear();
    if let Some(custom) = env_nonempty("WINMODING_CURSEFORGE_API") {
        extras.push(custom.trim_end_matches('/').to_string());
    }
    if let Some(mut expanded) = expand_from_prefixes(&url, CURSEFORGE_API_PREFIXES, &extras) {
        if has_curseforge_key(extra_headers) {
            if let Some(index) = expanded
                .iter()
                .position(|item| item.starts_with("https://api.curseforge.com/v1"))
            {
                let official = expanded.remove(index);
                expanded.insert(0, official);
            }
        }
        return expanded;
    }
    let mut out = vec![url.clone()];
    if let Some(mut expanded) = expand_from_prefixes(&url, MODRINTH_CDN_PREFIXES, &[]) {
        if let Some(proxied) = millida_file_proxy(&url) {
            push_unique(&mut expanded, proxied);
        }
        return expanded;
    }
    if url.contains("edge.forgecdn.net") {
        push_unique(
            &mut out,
            url.replace("edge.forgecdn.net", "mediafilez.forgecdn.net"),
        );
        push_unique(
            &mut out,
            url.replace("edge.forgecdn.net", "media.forgecdn.net"),
        );
    }
    if url.contains("mediafilez.forgecdn.net") {
        push_unique(
            &mut out,
            url.replace("mediafilez.forgecdn.net", "edge.forgecdn.net"),
        );
        push_unique(
            &mut out,
            url.replace("mediafilez.forgecdn.net", "media.forgecdn.net"),
        );
    }
    if url.contains("github.com") || url.contains("githubusercontent.com") {
        push_unique(&mut out, format!("https://ghfast.top/{url}"));
        push_unique(&mut out, format!("https://gh-proxy.com/{url}"));
        if let Some(rest) = url.strip_prefix("https://github.com") {
            push_unique(&mut out, format!("https://kkgithub.com{rest}"));
        }
    }
    if let Some(proxied) = millida_file_proxy(&url) {
        push_unique(&mut out, proxied);
    }
    out
}

fn trim_body(body: &str) -> String {
    let compact = body.trim();
    compact.chars().take(280).collect()
}

/// Run downloads with a shared concurrency ceiling.
pub async fn download_many(
    http: &HttpPool,
    jobs: Vec<DownloadJob>,
    concurrency: usize,
) -> Result<(), LauncherError> {
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
    let mut tasks = Vec::with_capacity(jobs.len());
    for job in jobs {
        let permit = semaphore.clone();
        let client = http.clone();
        tasks.push(tokio::spawn(async move {
            let _ticket = permit
                .acquire_owned()
                .await
                .map_err(|err| LauncherError::Message(err.to_string()))?;
            if let Some(hash) = &job.hash {
                client.download_verified(&job.url, &job.dest, hash).await
            } else {
                client.download_to(&job.url, &job.dest).await
            }
        }));
    }
    for task in tasks {
        match task.await {
            Ok(result) => result?,
            Err(err) => {
                return Err(LauncherError::Message(format!(
                    "download task failed: {err}"
                )))
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub dest: std::path::PathBuf,
    pub hash: Option<ContentHash>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_path_appends_suffix() {
        let dest = std::path::PathBuf::from("pack.jar");
        assert_eq!(
            partial_path(&dest)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("pack.jar.partial")
        );
    }

    #[test]
    fn partial_file_renames_to_dest() {
        let dir = std::env::temp_dir().join(format!("winmoding-net-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.bin");
        let partial = partial_path(&dest);
        std::fs::write(&partial, b"hello").unwrap();
        std::fs::rename(&partial, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        assert!(!partial.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn curseforge_without_key_tries_public_proxy_first() {
        let url = "https://api.curseforge.com/v1/mods/search?gameId=432";
        let order = alternate_urls(url, &[]);
        assert!(
            order[0].starts_with("https://api.millida.net/v2/launcher/cf/"),
            "RF CurseForge relay is first when no local API key"
        );
        assert!(order
            .iter()
            .any(|item| item.starts_with("https://api.curse.tools/v1/")));
        assert!(order.iter().any(|item| item.starts_with(url)));
        assert!(order
            .iter()
            .any(|item| item.contains("mod.mcimirror.top/curseforge/v1/")));
        assert!(order
            .iter()
            .any(|item| item.starts_with("https://cfproxy.fly.dev/v1/")));
    }

    #[test]
    fn curseforge_with_key_keeps_official_first() {
        let url = "https://api.curseforge.com/v1/mods/search?gameId=432";
        let order = alternate_urls(url, &[("x-api-key", "secret")]);
        assert_eq!(order[0], url);
        assert!(order
            .iter()
            .any(|item| item.starts_with("https://api.curse.tools/v1/")));
    }

    #[test]
    fn modrinth_picks_official_and_mirrors() {
        let url = "https://api.modrinth.com/v2/search?query=sodium";
        let order = alternate_urls(url, &[]);
        assert!(order.iter().any(|item| item.starts_with(url)));
        assert!(order
            .iter()
            .any(|item| item.contains("staging-api.modrinth.com")));
        assert!(order.iter().any(|item| item.starts_with(
            "https://api.millida.net/v2/launcher/mr/v2/search?query=sodium"
        )));
        assert!(order
            .iter()
            .any(|item| item.contains("mod.mcimirror.top/modrinth/v2/search")));
    }

    #[test]
    fn millida_relays_blocked_files_only() {
        let fabric = "https://meta.fabricmc.net/v2/versions/loader";
        let order = alternate_urls(fabric, &[]);
        assert!(order.iter().any(|item| item.starts_with(fabric)));
        assert!(order.iter().any(|item| item.starts_with(
            "https://api.millida.net/v2/launcher/dl?url="
        )));
        let session = "https://sessionserver.mojang.com/session/minecraft/profile/x";
        assert_eq!(alternate_urls(session, &[]), vec![session.to_string()]);
        let evil = "https://evil.example/cdn.modrinth.com/x.jar";
        assert_eq!(alternate_urls(evil, &[]), vec![evil.to_string()]);
    }

    #[test]
    fn unavailable_mirror_is_503() {
        let err = LauncherError::Provider {
            provider: "mod.mcimirror.top".into(),
            status: 503,
            body: String::new(),
        };
        assert!(is_unavailable_mirror(&err));
        assert!(!is_connect_like(&err));
    }

    #[test]
    fn modrinth_cdn_has_alt_host() {
        let url = "https://cdn.modrinth.com/data/x/y.jar";
        let order = alternate_urls(url, &[]);
        assert_eq!(order[0], url);
        assert!(order
            .iter()
            .any(|item| item.starts_with("https://cdn-alt.modrinth.com/")));
        assert!(order.iter().any(|item| item.contains("api.millida.net/v2/launcher/dl?url=")));
    }

    #[test]
    fn windows_proxy_server_parses_v2ray_and_clash() {
        assert_eq!(
            parse_windows_proxy_server("127.0.0.1:10809").as_deref(),
            Some("http://127.0.0.1:10809")
        );
        assert_eq!(
            parse_windows_proxy_server("http=127.0.0.1:7890;https=127.0.0.1:7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(
            parse_windows_proxy_server("socks=127.0.0.1:10808").as_deref(),
            Some("socks5h://127.0.0.1:10808")
        );
        assert_eq!(parse_windows_proxy_server("").as_deref(), None);
        assert_eq!(
            v2ray_socks_sibling("http://127.0.0.1:10809").as_deref(),
            Some("socks5h://127.0.0.1:10808")
        );
    }

    #[test]
    fn http_pool_builds() {
        HttpPool::new().expect("http client");
    }

    #[test]
    fn skipped_hosts_are_dropped_when_live_remain() {
        let err = LauncherError::Provider {
            provider: "mod.mcimirror.top".into(),
            status: 503,
            body: String::new(),
        };
        mark_skip_from_err("https://mod.mcimirror.top/modrinth/v2/search", &err);
        let kept = usable_candidates(vec![
            "https://mod.mcimirror.top/modrinth/v2/search".into(),
            "https://api.millida.net/v2/launcher/mr/v2/search".into(),
        ]);
        assert_eq!(kept.len(), 1);
        assert!(kept[0].contains("millida.net"));
    }

    #[tokio::test]
    #[ignore = "hits live Modrinth / curse.tools"]
    async fn live_modrinth_and_curse_tools_search() {
        let http = HttpPool::new().expect("http client");
        let modrinth = http
            .get_text(
                "https://api.modrinth.com/v2/search?query=sodium&limit=1",
                &[],
            )
            .await
            .expect("modrinth official search");
        assert!(
            modrinth.contains("hits") || modrinth.contains("project_id"),
            "unexpected modrinth payload"
        );
        let curse = http
            .get_text(
                "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&pageSize=1&index=0",
                &[],
            )
            .await
            .expect("curse.tools search");
        assert!(
            curse.contains("data") || curse.contains("id"),
            "unexpected curseforge payload"
        );
    }
}
