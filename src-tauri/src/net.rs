use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, USER_AGENT};
use tokio::io::AsyncWriteExt;

use crate::cache::MetadataCache;
use crate::domain::ContentHash;
use crate::error::LauncherError;
use crate::hashutil::verify_bytes;
use crate::protocol::USER_AGENT as APP_USER_AGENT;

const MAX_ATTEMPTS: u32 = 3;
const API_CACHE_TTL_SECS: i64 = 600;

#[derive(Clone)]
pub struct HttpPool {
    client: reqwest::Client,
}

impl HttpPool {
    pub fn new() -> Result<Self, LauncherError> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .user_agent(APP_USER_AGENT)
            .gzip(true)
            .build()
            .map_err(|source| LauncherError::Network {
                url: "client-init".into(),
                source,
            })?;
        Ok(Self { client })
    }

    pub async fn get_text(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, LauncherError> {
        let mut last = None;
        for candidate in alternate_urls(url) {
            match self.get_text_once(&candidate, extra_headers).await {
                Ok(body) => return Ok(body),
                Err(err) if err.should_try_mirror() => last = Some(err),
                Err(err) => return Err(err),
            }
        }
        Err(last.unwrap_or_else(|| LauncherError::Message(format!("request failed: {url}"))))
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
        if let Some(hit) = cache.cached_body(key).await? {
            return Ok(hit);
        }
        let body = self.get_text(url, extra_headers).await?;
        cache.store_body(key, &body, API_CACHE_TTL_SECS).await?;
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
        for candidate in alternate_urls(url) {
            match self.download_to_once(&candidate, dest).await {
                Ok(()) => return Ok(()),
                Err(err) if err.should_try_mirror() => last = Some(err),
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
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|source| LauncherError::Network {
                    url: url.to_string(),
                    source,
                })?;
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
        for candidate in alternate_urls(url) {
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
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|source| LauncherError::Network {
                    url: url.to_string(),
                    source,
                })?;
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
        self.client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|source| LauncherError::Network {
                url: url.to_string(),
                source,
            })
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

pub fn partial_path(dest: &Path) -> std::path::PathBuf {
    let mut name = dest
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_else(|| "download".into());
    name.push(".partial");
    dest.with_file_name(name)
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

fn alternate_urls(url: &str) -> Vec<String> {
    let mut out = vec![url.to_string()];
    if let Some(rest) = url.strip_prefix("https://api.modrinth.com/v2") {
        push_unique(
            &mut out,
            format!("https://mod.mcimirror.top/modrinth/v2{rest}"),
        );
    }
    if let Some(rest) = url.strip_prefix("https://cdn.modrinth.com") {
        push_unique(
            &mut out,
            format!("https://mod.mcimirror.top/modrinth{rest}"),
        );
    }
    if let Some(rest) = url.strip_prefix("https://api.curseforge.com/v1") {
        push_unique(
            &mut out,
            format!("https://mod.mcimirror.top/curseforge/v1{rest}"),
        );
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
}
