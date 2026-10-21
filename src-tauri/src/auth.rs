use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use md5::{Digest, Md5};
use serde::Deserialize;
use serde::Serialize;
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::domain::StoredAccount;
use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::protocol::{
    ELY_AUTHENTICATE, MC_LOGIN_XBOX, MC_PROFILE, MS_AUTHORIZE, MS_DESKTOP_REDIRECT, MS_TOKEN,
    MS_XBOX_PUBLIC_CLIENT, XBL_AUTHENTICATE, XSTS_AUTHORIZE,
};

const AUTH_TIMEOUT_SECS: u64 = 180;

pub fn offline_uuid(username: &str) -> String {
    let mut digest = Md5::new();
    digest.update(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = digest.finalize();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub fn offline_account(username: &str) -> Result<StoredAccount, LauncherError> {
    let trimmed = username.trim();
    let valid = (3..=16).contains(&trimmed.len())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if !valid {
        return Err(LauncherError::Auth {
            reason: "offline name must be 3-16 characters of letters, digits, or underscore".into(),
        });
    }
    let uuid = offline_uuid(trimmed);
    Ok(StoredAccount {
        id: format!("offline-{uuid}"),
        kind: "offline".into(),
        username: trimmed.to_string(),
        uuid,
        access_token: "0".into(),
        refresh_token: None,
        expires_unix: None,
    })
}

pub async fn login_microsoft(
    http: &HttpPool,
    client_id: &str,
) -> Result<StoredAccount, LauncherError> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(LauncherError::Auth {
            reason: "Microsoft sign-in needs an Azure client id. Paste it in Settings в†’ Minecraft & CurseForge, or use Offline login.".into(),
        });
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| crate::error::io_at("127.0.0.1:0", err))?;
    let port = listener
        .local_addr()
        .map_err(|err| crate::error::io_at("127.0.0.1", err))?
        .port();
    let redirect = format!("http://127.0.0.1:{port}/callback");
    let (verifier, challenge) = pkce_pair();
    let state = Uuid::new_v4().to_string();
    let url = format!(
        "{MS_AUTHORIZE}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&prompt=select_account",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode("XboxLive.signin offline_access"),
        urlencoding::encode(&state),
        urlencoding::encode(&challenge)
    );
    open_browser(&url)?;
    let callback = tokio::time::timeout(
        std::time::Duration::from_secs(AUTH_TIMEOUT_SECS),
        accept_callback(listener, &state),
    )
    .await
    .map_err(|_| LauncherError::Auth {
        reason: "Microsoft login timed out".into(),
    })??;
    let token: MsToken = http
        .post_form(
            MS_TOKEN,
            &[
                ("client_id", client_id),
                ("grant_type", "authorization_code"),
                ("code", callback.code.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("code_verifier", verifier.as_str()),
            ],
        )
        .await?;
    exchange_xbox(http, token, true).await
}

/// Official Minecraft public-client flow (Xbox MBI_SSL + desktop redirect).
/// Opens a WinModing window, waits for `oauth20_desktop.srf?code=`, no Azure app needed.
pub async fn login_microsoft_official(
    app: &tauri::AppHandle,
    http: &HttpPool,
) -> Result<StoredAccount, LauncherError> {
    use tauri::Manager;

    let url = format!(
        "{MS_AUTHORIZE}?client_id={}&response_type=code&redirect_uri={}&scope={}&display=touch&prompt=select_account",
        urlencoding::encode(MS_XBOX_PUBLIC_CLIENT),
        urlencoding::encode(MS_DESKTOP_REDIRECT),
        urlencoding::encode("service::user.auth.xboxlive.com::MBI_SSL")
    );
    let parsed = reqwest::Url::parse(&url).map_err(|err| LauncherError::Auth {
        reason: format!("bad Microsoft login url: {err}"),
    })?;
    if let Some(existing) = app.get_webview_window("msa-login") {
        let _ = existing.close();
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "msa-login",
        tauri::WebviewUrl::External(parsed),
    )
    .title("Microsoft — WinModing")
    .inner_size(520.0, 800.0)
    .center()
    .focused(true)
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
    .build()
    .map_err(|err| LauncherError::Auth {
        reason: format!("could not open Microsoft login: {err}"),
    })?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(AUTH_TIMEOUT_SECS);
    let code = loop {
        if tokio::time::Instant::now() > deadline {
            let _ = window.close();
            return Err(LauncherError::Auth {
                reason: "Microsoft login timed out".into(),
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        let current = match window.url() {
            Ok(url) => url,
            Err(_) => {
                return Err(LauncherError::Auth {
                    reason: "Microsoft login window was closed".into(),
                });
            }
        };
        let href = current.as_str();
        if href.starts_with(MS_DESKTOP_REDIRECT) {
            let query = current.query().unwrap_or("");
            let error = query_param(query, "error");
            let code = query_param(query, "code");
            let _ = window.close();
            if let Some(error) = error {
                return Err(LauncherError::Auth {
                    reason: format!("Microsoft returned {error}"),
                });
            }
            break code.ok_or_else(|| LauncherError::Auth {
                reason: "Microsoft login did not return a code".into(),
            })?;
        }
    };
    let token: MsToken = http
        .post_form(
            MS_TOKEN,
            &[
                ("client_id", MS_XBOX_PUBLIC_CLIENT),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", MS_DESKTOP_REDIRECT),
            ],
        )
        .await?;
    exchange_xbox(http, token, false).await
}

pub async fn login_ely(
    http: &HttpPool,
    username: &str,
    password: &str,
) -> Result<StoredAccount, LauncherError> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err(LauncherError::Auth {
            reason: "Ely.by needs a username and password".into(),
        });
    }
    let body = serde_json::json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": username,
        "password": password,
        "clientToken": Uuid::new_v4().to_string(),
        "requestUser": true,
    });
    let response: ElyAuthResponse =
        http.post_json(ELY_AUTHENTICATE, &body)
            .await
            .map_err(|_| LauncherError::Auth {
                reason: "Ely.by rejected the login. Check username and password.".into(),
            })?;
    if let Some(message) = response.error_message.or(response.error) {
        return Err(LauncherError::Auth { reason: message });
    }
    let profile = response
        .selected_profile
        .ok_or_else(|| LauncherError::Auth {
            reason: "Ely.by did not return a Minecraft profile".into(),
        })?;
    let access_token = response.access_token.ok_or_else(|| LauncherError::Auth {
        reason: "Ely.by did not return an access token".into(),
    })?;
    Ok(StoredAccount {
        id: format!("ely-{}", profile.id),
        kind: "ely".into(),
        username: profile.name,
        uuid: dashed_uuid(&profile.id),
        access_token,
        refresh_token: None,
        expires_unix: None,
    })
}

fn dashed_uuid(id: &str) -> String {
    let hex: String = id.chars().filter(|ch| *ch != '-').collect();
    if hex.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    } else {
        id.to_string()
    }
}

async fn exchange_xbox(
    http: &HttpPool,
    token: MsToken,
    prefix_d: bool,
) -> Result<StoredAccount, LauncherError> {
    let rps_ticket = if prefix_d {
        format!("d={}", token.access_token)
    } else {
        token.access_token.clone()
    };
    let xbl: XboxResponse = http
        .post_json(
            XBL_AUTHENTICATE,
            &XboxRequest {
                properties: XboxProperties {
                    auth_method: "RPS",
                    site_name: "user.auth.xboxlive.com",
                    rps_ticket,
                },
                relying_party: "http://auth.xboxlive.com",
                token_type: "JWT",
            },
        )
        .await?;
    let uhs = xbl
        .display_claims
        .xui
        .first()
        .map(|claim| claim.uhs.clone())
        .ok_or_else(|| LauncherError::Auth {
            reason: "Xbox response did not include a user hash".into(),
        })?;
    let xsts: XboxResponse = http
        .post_json(
            XSTS_AUTHORIZE,
            &XstsRequest {
                properties: XstsProperties {
                    sandbox_id: "RETAIL",
                    user_tokens: vec![xbl.token.clone()],
                },
                relying_party: "rp://api.minecraftservices.com/",
                token_type: "JWT",
            },
        )
        .await?;
    let identity = format!("XBL3.0 x={uhs};{}", xsts.token);
    let minecraft: MinecraftLogin = http
        .post_json(
            MC_LOGIN_XBOX,
            &MinecraftLoginRequest {
                identity_token: identity,
            },
        )
        .await?;
    let bearer = format!("Bearer {}", minecraft.access_token);
    let profile: MinecraftProfile = http
        .get_json(MC_PROFILE, &[("Authorization", bearer.as_str())])
        .await
        .map_err(|_| LauncherError::Auth {
            reason: "Minecraft profile is unavailable. The account may not own the game.".into(),
        })?;
    let expires = unix_now().saturating_add(token.expires_in);
    Ok(StoredAccount {
        id: format!("msa-{}", profile.id),
        kind: "microsoft".into(),
        username: profile.name,
        uuid: profile.id,
        access_token: minecraft.access_token,
        refresh_token: token.refresh_token,
        expires_unix: Some(expires as i64),
    })
}

fn pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

async fn accept_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<CallbackQuery, LauncherError> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|err| crate::error::io_at("oauth-callback", err))?;
    let mut buffer = vec![0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .map_err(|err| crate::error::io_at("oauth-callback", err))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first = request.lines().next().unwrap_or("");
    let target = first.split_whitespace().nth(1).unwrap_or("/");
    let query = target.split_once('?').map(|(_, query)| query).unwrap_or("");
    let code = query_param(query, "code");
    let state = query_param(query, "state");
    let error = query_param(query, "error");
    let body = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><title>WinModing</title><p>Login captured. You can close this tab.</p>";
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.shutdown().await;
    if let Some(error) = error {
        return Err(LauncherError::Auth {
            reason: format!("Microsoft returned {error}"),
        });
    }
    if state.as_deref() != Some(expected_state) {
        return Err(LauncherError::Auth {
            reason: "OAuth state did not match".into(),
        });
    }
    let code = code.ok_or_else(|| LauncherError::Auth {
        reason: "OAuth callback did not include a code".into(),
    })?;
    Ok(CallbackQuery { code })
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        if name != key {
            return None;
        }
        urlencoding::decode(value)
            .ok()
            .map(|decoded| decoded.into_owned())
    })
}

fn open_browser(url: &str) -> Result<(), LauncherError> {
    crate::window::open_with_os(url).map_err(|err| LauncherError::Auth {
        reason: format!("could not open the browser: {err}"),
    })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

struct CallbackQuery {
    code: String,
}

#[derive(Debug, Deserialize)]
struct MsToken {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxRequest<'a> {
    properties: XboxProperties<'a>,
    relying_party: &'a str,
    token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxProperties<'a> {
    auth_method: &'a str,
    site_name: &'a str,
    rps_ticket: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XstsRequest<'a> {
    properties: XstsProperties,
    relying_party: &'a str,
    token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XstsProperties {
    sandbox_id: &'static str,
    user_tokens: Vec<String>,
}

#[derive(Deserialize)]
struct XboxResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<XuiClaim>,
}

#[derive(Deserialize)]
struct XuiClaim {
    uhs: String,
}

#[derive(Serialize)]
struct MinecraftLoginRequest {
    #[serde(rename = "identityToken")]
    identity_token: String,
}

#[derive(Deserialize)]
struct MinecraftLogin {
    access_token: String,
}

#[derive(Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ElyAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<ElyProfile>,
    error: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ElyProfile {
    id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notch_offline_uuid_matches_minecraft_vector() {
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn offline_name_rejects_spaces() {
        assert!(offline_account("bad name").is_err());
        assert!(offline_account("Steve").is_ok());
    }
}
