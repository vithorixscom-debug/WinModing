//! Recommended multiplayer servers.
//! Prefers Modrinth Server Projects (via MR relay), falls back to the public
//! rating catalogue when Modrinth has no hits yet.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::protocol::MODRINTH_API;

const RATING_SERVERS: &str = "https://api.millida.net/v2/rating/servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedServer {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub address: String,
    pub description: String,
    pub tags: Vec<String>,
    pub versions: Vec<String>,
    pub version_min: Option<String>,
    pub version_max: Option<String>,
    pub online: Option<i64>,
    pub is_online: Option<bool>,
    pub logo_url: Option<String>,
    pub banner_url: Option<String>,
    pub website_url: Option<String>,
    pub discord_url: Option<String>,
    pub license: Option<String>,
    pub edition: Option<String>,
    pub features: Vec<String>,
    pub categories: Vec<String>,
    pub modpack_ids: Vec<String>,
    pub source: String,
    pub motd: Option<String>,
    pub join_instructions: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersPage {
    pub servers: Vec<RecommendedServer>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
    pub source: String,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn i64_field(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| {
        x.as_i64()
            .or_else(|| x.as_f64().map(|n| n as i64))
            .or_else(|| x.as_str()?.parse().ok())
    })
}

fn bool_field(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|x| x.as_bool())
}

fn string_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn join_address(ip: &str, port: Option<u16>) -> String {
    let ip = ip.trim();
    if ip.is_empty() {
        return String::new();
    }
    match port {
        Some(p) if p != 0 && p != 25565 && !ip.contains(':') => format!("{ip}:{p}"),
        _ => ip.to_string(),
    }
}

fn from_rating(raw: &Value) -> Option<RecommendedServer> {
    let edition = str_field(raw, "edition").unwrap_or_else(|| "JAVA".into());
    if !edition.eq_ignore_ascii_case("JAVA") {
        return None;
    }
    let ip = str_field(raw, "ip")
        .or_else(|| str_field(raw, "address"))
        .unwrap_or_default();
    if ip.is_empty() {
        return None;
    }
    let port = i64_field(raw, "port")
        .or_else(|| i64_field(raw, "srvPort"))
        .and_then(|p| u16::try_from(p).ok());
    let name = str_field(raw, "name").unwrap_or_else(|| ip.clone());
    let slug = str_field(raw, "slug").unwrap_or_else(|| name.to_ascii_lowercase());
    let id = str_field(raw, "id").unwrap_or_else(|| slug.clone());
    let description = str_field(raw, "shortDesc")
        .or_else(|| str_field(raw, "aiDescription"))
        .or_else(|| str_field(raw, "description"))
        .unwrap_or_default();
    let mut categories = string_list(raw, "categories");
    let mut features = string_list(raw, "features");
    let mut tags = string_list(raw, "aiTags");
    tags.extend(categories.iter().cloned());
    let versions = {
        let majors = string_list(raw, "versionMajors");
        if !majors.is_empty() {
            majors
        } else {
            string_list(raw, "versions")
        }
    };
    let modpack_ids = raw
        .get("modpacks")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    str_field(m, "id")
                        .or_else(|| str_field(m, "projectId"))
                        .or_else(|| m.as_str().map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default();
    if categories.is_empty() {
        categories = tags.clone();
    }
    if features.is_empty() {
        features = string_list(raw, "features");
    }
    Some(RecommendedServer {
        id,
        name,
        slug,
        address: join_address(&ip, port),
        description,
        tags,
        versions,
        version_min: str_field(raw, "versionMin"),
        version_max: str_field(raw, "versionMax"),
        online: i64_field(raw, "online").or_else(|| i64_field(raw, "avgOnline")),
        is_online: bool_field(raw, "isOnline"),
        logo_url: str_field(raw, "logoUrl"),
        banner_url: str_field(raw, "bannerUrl"),
        website_url: str_field(raw, "websiteUrl"),
        discord_url: str_field(raw, "discordInviteUrl"),
        license: str_field(raw, "license"),
        edition: Some(edition),
        features,
        categories,
        modpack_ids,
        source: "catalog".into(),
        motd: str_field(raw, "motd"),
        join_instructions: str_field(raw, "joinInstructions"),
        country: str_field(raw, "country"),
        language: str_field(raw, "language"),
    })
}

fn from_modrinth_hit(raw: &Value) -> Option<RecommendedServer> {
    let project_type = str_field(raw, "project_type").unwrap_or_default();
    let all_types = string_list(raw, "all_project_types");
    let is_server = project_type.eq_ignore_ascii_case("server")
        || all_types.iter().any(|t| t.eq_ignore_ascii_case("server"));
    if !is_server {
        return None;
    }
    let id = str_field(raw, "project_id")
        .or_else(|| str_field(raw, "slug"))
        .unwrap_or_default();
    if id.is_empty() {
        return None;
    }
    let name = str_field(raw, "title").unwrap_or_else(|| id.clone());
    let slug = str_field(raw, "slug").unwrap_or_else(|| id.clone());
    let description = str_field(raw, "description").unwrap_or_default();
    let categories = string_list(raw, "categories");
    let display = string_list(raw, "display_categories");
    let mut tags = categories.clone();
    tags.extend(display);
    let versions = string_list(raw, "versions");
    // Server Projects store join address in gallery/additional fields when present.
    let address = str_field(raw, "java_address")
        .or_else(|| str_field(raw, "address"))
        .or_else(|| str_field(raw, "ip"))
        .or_else(|| {
            raw.get("gallery")
                .and_then(|g| g.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| str_field(item, "url"))
                .filter(|u| !u.starts_with("http"))
        })
        .unwrap_or_default();
    let website_url = str_field(raw, "project_url")
        .or_else(|| Some(format!("https://modrinth.com/server/{slug}")));
    Some(RecommendedServer {
        id: id.clone(),
        name,
        slug,
        address,
        description,
        tags,
        versions,
        version_min: None,
        version_max: None,
        online: i64_field(raw, "follows").or_else(|| i64_field(raw, "downloads")),
        is_online: None,
        logo_url: str_field(raw, "icon_url"),
        banner_url: raw
            .get("gallery")
            .and_then(|g| g.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| {
                if let Some(s) = item.as_str() {
                    Some(s.to_string())
                } else {
                    str_field(item, "url")
                }
            }),
        website_url,
        discord_url: None,
        license: str_field(raw, "license"),
        edition: Some("JAVA".into()),
        features: Vec::new(),
        categories,
        modpack_ids: Vec::new(),
        source: "modrinth".into(),
        motd: None,
        join_instructions: None,
        country: str_field(raw, "country"),
        language: str_field(raw, "language"),
    })
}

async fn fetch_modrinth_servers(
    http: &HttpPool,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<ServersPage, LauncherError> {
    let facets = urlencoding::encode(r#"[["project_type:server"]]"#);
    let q = urlencoding::encode(query.trim());
    let url = format!(
        "{MODRINTH_API}/search?query={q}&limit={limit}&offset={offset}&index=relevance&facets={facets}"
    );
    let body: Value = http.get_json(&url, &[]).await?;
    let hits = body
        .get("hits")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();
    let servers: Vec<RecommendedServer> = hits.iter().filter_map(from_modrinth_hit).collect();
    let total = body
        .get("total_hits")
        .and_then(|t| t.as_u64())
        .unwrap_or(servers.len() as u64);
    Ok(ServersPage {
        total,
        limit,
        offset,
        source: "modrinth".into(),
        servers,
    })
}

async fn fetch_rating_servers(
    http: &HttpPool,
    query: &str,
    limit: u32,
    offset: u32,
    sort: &str,
    category: &str,
    version: &str,
) -> Result<ServersPage, LauncherError> {
    let mut url = format!("{RATING_SERVERS}?limit={limit}&offset={offset}&sort={sort}");
    if !category.trim().is_empty() {
        url.push_str(&format!(
            "&category={}",
            urlencoding::encode(category.trim())
        ));
    }
    if !version.trim().is_empty() {
        url.push_str(&format!(
            "&version={}",
            urlencoding::encode(&version.trim()[..version.trim().len().min(16)])
        ));
    }
    if !query.trim().is_empty() {
        url.push_str(&format!(
            "&search={}",
            urlencoding::encode(&query.trim()[..query.trim().len().min(60)])
        ));
    }
    let body: Value = http.get_json(&url, &[]).await.map_err(|err| {
        // Refresh proxies once — RF IPs often need the system proxy hop.
        LauncherError::Message(err.to_string())
    })?;
    let rows = body
        .get("servers")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let servers: Vec<RecommendedServer> = rows.iter().filter_map(from_rating).collect();
    let total = body
        .get("total")
        .and_then(|t| t.as_u64())
        .unwrap_or(servers.len() as u64);
    Ok(ServersPage {
        total,
        limit,
        offset,
        source: "catalog".into(),
        servers,
    })
}

pub async fn list_recommended_servers(
    http: &HttpPool,
    query: String,
    limit: Option<u32>,
    offset: Option<u32>,
    sort: Option<String>,
    category: Option<String>,
    version: Option<String>,
) -> Result<ServersPage, LauncherError> {
    let limit = limit.unwrap_or(40).clamp(1, 60);
    let offset = offset.unwrap_or(0);
    let sort = sort.unwrap_or_else(|| "rating".into());
    let category = category.unwrap_or_default();
    let version = version.unwrap_or_default();

    // Rating catalogue first (works on RU via api.millida.net). Modrinth Server
    // Projects are still rare / empty on many mirrors — try them only as a bonus
    // merge when the first page is empty.
    let rating = match fetch_rating_servers(http, &query, limit, offset, &sort, &category, &version)
        .await
    {
        Ok(page) => page,
        Err(_) => {
            // Never fail the whole command — frontend keeps curated RF/abroad list.
            match http.refresh_proxies() {
                Ok(fresh) => fetch_rating_servers(
                    &fresh, &query, limit, offset, &sort, &category, &version,
                )
                .await
                .unwrap_or_else(|_| ServersPage {
                    total: 0,
                    limit,
                    offset,
                    source: "catalog".into(),
                    servers: Vec::new(),
                }),
                Err(_) => ServersPage {
                    total: 0,
                    limit,
                    offset,
                    source: "catalog".into(),
                    servers: Vec::new(),
                },
            }
        }
    };
    if !rating.servers.is_empty() || offset > 0 {
        return Ok(rating);
    }

    if let Ok(page) = fetch_modrinth_servers(http, &query, limit, offset).await {
        if !page.servers.is_empty() {
            return Ok(page);
        }
    }
    Ok(rating)
}
