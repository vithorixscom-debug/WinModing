use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use async_trait::async_trait;
use serde::Deserialize;

use crate::cache::MetadataCache;
use crate::domain::{
    normalize_title, CatalogPage, CatalogQuery, ContentHash, ProviderSource, ResolvedFile,
    UnifiedMod,
};
use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::protocol::{
    curseforge_class_id, curseforge_headers, curseforge_loader_type, normalize_mod_loader,
    parse_github_repo, CURSEFORGE_API, CURSEFORGE_ID_PREFIX, CURSEFORGE_MINECRAFT_GAME_ID,
    MODRINTH_API,
};

const KNOWN_LOADERS: &[&str] = &["fabric", "forge", "neoforge", "quilt"];

#[async_trait]
pub trait ModProvider: Send + Sync {
    fn source(&self) -> ProviderSource;
    async fn search(&self, query: &CatalogQuery) -> Result<(Vec<UnifiedMod>, u64), LauncherError>;
}

#[derive(Clone)]
pub struct ModrinthProvider {
    http: HttpPool,
    cache: MetadataCache,
}

#[derive(Clone)]
pub struct CurseForgeProvider {
    http: HttpPool,
    cache: MetadataCache,
    api_key: String,
}

impl ModrinthProvider {
    pub fn new(http: HttpPool, cache: MetadataCache) -> Self {
        Self { http, cache }
    }
}

impl CurseForgeProvider {
    pub fn new(http: HttpPool, cache: MetadataCache, api_key: String) -> Self {
        Self {
            http,
            cache,
            api_key,
        }
    }
}

#[async_trait]
impl ModProvider for ModrinthProvider {
    fn source(&self) -> ProviderSource {
        ProviderSource::Modrinth
    }

    async fn search(&self, query: &CatalogQuery) -> Result<(Vec<UnifiedMod>, u64), LauncherError> {
        let url = modrinth_search_url(query)?;
        let body = self
            .http
            .cached_text(&self.cache, &format!("modrinth:{url}"), &url, &[])
            .await?;
        let parsed: ModrinthSearch =
            serde_json::from_str(&body).map_err(|err| LauncherError::Provider {
                provider: "modrinth".into(),
                status: 200,
                body: format!("unreadable search payload: {err}"),
            })?;
        let total = parsed.total_hits.max(parsed.hits.len() as u64);
        Ok((
            parsed
                .hits
                .into_iter()
                .map(|hit| UnifiedMod {
                    id: hit.project_id,
                    source: ProviderSource::Modrinth,
                    name: hit.title,
                    summary: hit.description,
                    downloads: hit.downloads,
                    follows: hit.follows,
                    date_modified: hit.date_modified.filter(|value| !value.is_empty()),
                    client_side: hit.client_side.filter(|value| !value.is_empty()),
                    server_side: hit.server_side.filter(|value| !value.is_empty()),
                    icon_url: hit.icon_url,
                    supported_loaders: loaders_from_categories(&hit.categories),
                    supported_game_versions: hit.versions,
                    slug: hit.slug,
                    author: Some(hit.author),
                    also_listed_on: None,
                })
                .collect(),
            total,
        ))
    }
}

#[async_trait]
impl ModProvider for CurseForgeProvider {
    fn source(&self) -> ProviderSource {
        ProviderSource::CurseForge
    }

    async fn search(&self, query: &CatalogQuery) -> Result<(Vec<UnifiedMod>, u64), LauncherError> {
        let wanted = query.limit.max(1);
        let mut items = Vec::new();
        let mut offset = query.offset;
        let mut total = 0u64;
        let auth = curseforge_headers(&self.api_key);
        while (items.len() as u32) < wanted {
            let mut slice = query.clone();
            slice.offset = offset;
            slice.limit = (wanted - items.len() as u32).min(50);
            let url = curseforge_search_url(&slice);
            let body = self
                .http
                .cached_text(&self.cache, &format!("curseforge:{url}"), &url, &auth)
                .await?;
            let parsed: CfSearch =
                serde_json::from_str(&body).map_err(|err| LauncherError::Provider {
                    provider: "curseforge".into(),
                    status: 200,
                    body: format!("unreadable search payload: {err}"),
                })?;
            total = parsed
                .pagination
                .as_ref()
                .map(|page| page.total_count)
                .unwrap_or(parsed.data.len() as u64)
                .max(parsed.data.len() as u64)
                .max(total);
            if parsed.data.is_empty() {
                break;
            }
            let batch_len = parsed.data.len() as u32;
            offset = offset.saturating_add(batch_len);
            items.extend(parsed.data.into_iter().map(cf_mod_to_unified));
            if batch_len < slice.limit {
                break;
            }
        }
        items.truncate(wanted as usize);
        let count = items.len() as u64;
        Ok((items, total.max(count)))
    }
}

pub async fn search_unified(
    http: &HttpPool,
    cache: &MetadataCache,
    query: &CatalogQuery,
    curseforge_key: &str,
) -> Result<CatalogPage, LauncherError> {
    let original_text = query.text.clone();
    let mut query = query.clone();
    if let Some((_, repo)) = parse_github_repo(&original_text) {
        query.text = repo;
    }
    query.loader = query.loader.as_deref().and_then(normalize_mod_loader);
    let source = query
        .source
        .as_deref()
        .unwrap_or(if query.show_both { "both" } else { "modrinth" })
        .to_ascii_lowercase();
    let want_modrinth = matches!(source.as_str(), "modrinth" | "both" | "all" | "");
    let want_curseforge = matches!(source.as_str(), "curseforge" | "both" | "all" | "");
    let show_both = source == "both" || source == "all";
    let modrinth = ModrinthProvider::new(http.clone(), cache.clone());
    let curseforge =
        CurseForgeProvider::new(http.clone(), cache.clone(), curseforge_key.to_string());
    let (left, right) = join_catalog_providers(
        want_modrinth,
        want_curseforge,
        &modrinth,
        &curseforge,
        &query,
    )
    .await;
    let mut warnings = Vec::new();
    let mut mr_items = Vec::new();
    let mut cf_items = Vec::new();
    let mut mr_total = 0u64;
    let mut cf_total = 0u64;
    match left {
        Ok((rows, total)) => {
            mr_total = total;
            mr_items = rows;
        }
        Err(err) => warnings.push(format!("Modrinth: {err}")),
    }
    match right {
        Ok((rows, total)) => {
            cf_total = total;
            cf_items = rows;
        }
        Err(err) => warnings.push(format!("CurseForge: {err}")),
    }
    let pref = query
        .duplicate_pref
        .as_deref()
        .unwrap_or("both")
        .to_ascii_lowercase();
    let (mut items, total_hits) = if show_both {
        concat_catalog_page(
            &modrinth,
            &curseforge,
            &query,
            mr_items,
            mr_total,
            cf_items,
            cf_total,
            &mut warnings,
        )
        .await
    } else if want_curseforge {
        (cf_items, cf_total)
    } else {
        (mr_items, mr_total)
    };
    if show_both {
        items = annotate_page(items);
        if pref != "both" && pref != "all" {
            items = keep_preferred_source(items, &pref);
        }
    }
    if let Some((owner, repo)) = parse_github_repo(&original_text) {
        if !items
            .iter()
            .any(|item| item.slug.eq_ignore_ascii_case(&repo))
        {
            items.insert(
                0,
                UnifiedMod {
                    id: format!("https://github.com/{owner}/{repo}"),
                    source: ProviderSource::GitHub,
                    name: repo.clone(),
                    summary: format!("{owner}/{repo} on GitHub"),
                    downloads: 0,
                    follows: 0,
                    date_modified: None,
                    client_side: None,
                    server_side: None,
                    icon_url: None,
                    supported_loaders: Vec::new(),
                    supported_game_versions: Vec::new(),
                    slug: repo,
                    author: Some(owner),
                    also_listed_on: None,
                },
            );
        }
    }
    if items.is_empty() && !warnings.is_empty() {
        return Err(LauncherError::Message(warnings.join(" | ")));
    }
    Ok(CatalogPage {
        total_hits: total_hits.max(items.len() as u64),
        items,
        warnings,
    })
}

enum CatalogSide {
    Modrinth(Result<(Vec<UnifiedMod>, u64), LauncherError>),
    CurseForge(Result<(Vec<UnifiedMod>, u64), LauncherError>),
}

async fn join_catalog_providers(
    want_modrinth: bool,
    want_curseforge: bool,
    modrinth: &ModrinthProvider,
    curseforge: &CurseForgeProvider,
    query: &CatalogQuery,
) -> (
    Result<(Vec<UnifiedMod>, u64), LauncherError>,
    Result<(Vec<UnifiedMod>, u64), LauncherError>,
) {
    if !want_modrinth && !want_curseforge {
        return (Ok((Vec::new(), 0)), Ok((Vec::new(), 0)));
    }
    let (tx, mut rx) = mpsc::channel::<CatalogSide>(2);
    if want_modrinth {
        let tx = tx.clone();
        let provider = modrinth.clone();
        let query = query.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(CatalogSide::Modrinth(search_relaxing(&provider, &query).await))
                .await;
        });
    }
    if want_curseforge {
        let tx = tx.clone();
        let provider = curseforge.clone();
        let query = query.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(CatalogSide::CurseForge(
                    search_relaxing(&provider, &query).await,
                ))
                .await;
        });
    }
    drop(tx);

    let mut left = if want_modrinth {
        None
    } else {
        Some(Ok((Vec::new(), 0)))
    };
    let mut right = if want_curseforge {
        None
    } else {
        Some(Ok((Vec::new(), 0)))
    };
    let started = tokio::time::Instant::now();
    while left.is_none() || right.is_none() {
        let had_one = left.is_some() || right.is_some();
        let wait = if had_one {
            Duration::from_millis(1500)
        } else {
            Duration::from_secs(6).saturating_sub(started.elapsed())
        };
        if wait.is_zero() {
            break;
        }
        match timeout(wait, rx.recv()).await {
            Ok(Some(CatalogSide::Modrinth(result))) => left = Some(result),
            Ok(Some(CatalogSide::CurseForge(result))) => right = Some(result),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    (
        left.unwrap_or_else(|| Err(LauncherError::Message("Modrinth timed out".into()))),
        right.unwrap_or_else(|| Err(LauncherError::Message("CurseForge timed out".into()))),
    )
}

async fn search_relaxing(
    provider: &impl ModProvider,
    query: &CatalogQuery,
) -> Result<(Vec<UnifiedMod>, u64), LauncherError> {
    let first = provider.search(query).await;
    let filtered = query
        .game_version
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || query
            .loader
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    match first {
        Ok((rows, total)) if !rows.is_empty() || !filtered => Ok((rows, total)),
        Ok((empty_rows, empty_total)) => {
            let mut relaxed = query.clone();
            relaxed.game_version = None;
            relaxed.loader = None;
            match provider.search(&relaxed).await {
                Ok((rows, total)) if !rows.is_empty() => Ok((rows, total)),
                Ok(_) => Ok((empty_rows, empty_total)),
                Err(_) => Ok((empty_rows, empty_total)),
            }
        }
        Err(err) => Err(err),
    }
}

fn annotate_page(mut items: Vec<UnifiedMod>) -> Vec<UnifiedMod> {
    let snapshot: Vec<(ProviderSource, String)> = items
        .iter()
        .map(|item| (item.source, normalize_title(&item.name)))
        .collect();
    for item in &mut items {
        if item.also_listed_on.is_some() {
            continue;
        }
        let key = normalize_title(&item.name);
        if let Some((source, _)) = snapshot
            .iter()
            .find(|(source, name)| *source != item.source && name == &key)
        {
            item.also_listed_on = Some(*source);
        }
    }
    items
}

fn cf_mod_to_unified(hit: CfMod) -> UnifiedMod {
    let author = hit.authors.first().map(|author| author.name.clone());
    let icon = hit.logo.and_then(|logo| logo.thumbnail_url.or(logo.url));
    let (versions, loaders) = versions_and_loaders(&hit.latest_files_indexes);
    UnifiedMod {
        id: format!("{CURSEFORGE_ID_PREFIX}{}", hit.id),
        source: ProviderSource::CurseForge,
        slug: hit.slug,
        name: hit.name,
        summary: hit.summary,
        downloads: hit.download_count,
        follows: hit.thumbs_up_count,
        date_modified: hit.date_modified.filter(|value| !value.is_empty()),
        client_side: None,
        server_side: None,
        icon_url: icon,
        supported_loaders: loaders,
        supported_game_versions: versions,
        author: author.or_else(|| Some("CurseForge".into())),
        also_listed_on: None,
    }
}

async fn concat_catalog_page(
    _modrinth: &ModrinthProvider,
    _curseforge: &CurseForgeProvider,
    query: &CatalogQuery,
    mr_items: Vec<UnifiedMod>,
    mr_total: u64,
    cf_items: Vec<UnifiedMod>,
    cf_total: u64,
    _warnings: &mut Vec<String>,
) -> (Vec<UnifiedMod>, u64) {
    let limit = query.limit.max(1) as usize;
    let total = mr_total.saturating_add(cf_total);
    (
        interleave_catalog(mr_items, cf_items, limit),
        total.max(mr_total).max(cf_total),
    )
}

fn interleave_catalog(
    mr_items: Vec<UnifiedMod>,
    cf_items: Vec<UnifiedMod>,
    limit: usize,
) -> Vec<UnifiedMod> {
    let mut out = Vec::with_capacity(limit.min(mr_items.len() + cf_items.len()));
    let mut left = mr_items.into_iter();
    let mut right = cf_items.into_iter();
    while out.len() < limit {
        let mut progressed = false;
        if let Some(item) = left.next() {
            out.push(item);
            progressed = true;
        }
        if out.len() >= limit {
            break;
        }
        if let Some(item) = right.next() {
            out.push(item);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    out
}

fn keep_preferred_source(items: Vec<UnifiedMod>, pref: &str) -> Vec<UnifiedMod> {
    let want_cf = matches!(pref, "curseforge" | "curse");
    items
        .into_iter()
        .filter(|item| {
            if want_cf {
                item.source == ProviderSource::CurseForge || item.also_listed_on.is_none()
            } else {
                item.source == ProviderSource::Modrinth || item.also_listed_on.is_none()
            }
        })
        .collect()
}

pub fn modrinth_search_url(query: &CatalogQuery) -> Result<String, LauncherError> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", query.project_type)]];
    if let Some(loader) = query
        .loader
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if loader != "vanilla" {
            facets.push(vec![format!("categories:{loader}")]);
        }
    }
    if let Some(game) = query
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        facets.push(vec![format!("versions:{game}")]);
    }
    let facets_json =
        serde_json::to_string(&facets).map_err(|err| LauncherError::Message(err.to_string()))?;
    let index = match query.index.as_deref().unwrap_or("relevance") {
        "downloads" => "downloads",
        "follows" => "follows",
        "newest" => "newest",
        "updated" => "updated",
        _ => "relevance",
    };
    Ok(format!(
        "{MODRINTH_API}/search?query={}&limit={}&offset={}&index={index}&facets={}",
        urlencoding::encode(query.text.trim()),
        query.limit.clamp(1, 100),
        query.offset,
        urlencoding::encode(&facets_json)
    ))
}

pub fn curseforge_search_url(query: &CatalogQuery) -> String {
    let class_id = curseforge_class_id(&query.project_type);
    let mut parts = vec![
        format!("gameId={CURSEFORGE_MINECRAFT_GAME_ID}"),
        format!("classId={class_id}"),
        format!("pageSize={}", query.limit.clamp(1, 50)),
        format!("index={}", query.offset),
        format!(
            "sortField={}",
            curseforge_sort_field(query.index.as_deref())
        ),
        "sortOrder=desc".to_string(),
    ];
    let text = query.text.trim();
    if !text.is_empty() {
        parts.push(format!("searchFilter={}", urlencoding::encode(text)));
    }
    if let Some(game) = query
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let encoded = serde_json::to_string(&[game]).unwrap_or_else(|_| format!("[\"{game}\"]"));
        parts.push(format!(
            "gameVersions={}",
            urlencoding::encode(&encoded)
        ));
    }
    if class_id == 6 || class_id == 4471 {
        if let Some(kind) = query.loader.as_deref().and_then(curseforge_loader_type) {
            let encoded = serde_json::to_string(&[kind]).unwrap_or_else(|_| format!("[{kind}]"));
            parts.push(format!(
                "modLoaderTypes={}",
                urlencoding::encode(&encoded)
            ));
        }
    }
    format!("{CURSEFORGE_API}/mods/search?{}", parts.join("&"))
}

fn curseforge_sort_field(index: Option<&str>) -> u8 {
    match index.unwrap_or("relevance") {
        "updated" => 3,
        "newest" => 11,
        "follows" => 12,
        "downloads" => 6,
        _ => 2,
    }
}

fn loaders_from_categories(categories: &[String]) -> Vec<String> {
    categories
        .iter()
        .filter(|category| KNOWN_LOADERS.contains(&category.as_str()))
        .cloned()
        .collect()
}

fn versions_and_loaders(indexes: &[CfFileIndex]) -> (Vec<String>, Vec<String>) {
    let mut versions = Vec::new();
    let mut loaders = Vec::new();
    for index in indexes {
        if let Some(version) = &index.game_version {
            if !versions.contains(version) {
                versions.push(version.clone());
            }
        }
        if let Some(name) = loader_name(index.mod_loader) {
            if !loaders.iter().any(|item| item == name) {
                loaders.push(name.to_string());
            }
        }
    }
    (versions, loaders)
}

fn loader_name(kind: Option<u8>) -> Option<&'static str> {
    match kind {
        Some(1) => Some("forge"),
        Some(4) => Some("fabric"),
        Some(5) => Some("quilt"),
        Some(6) => Some("neoforge"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct RemoteVersion {
    pub id: String,
    pub project_id: String,
    pub file_name: String,
    pub version_number: String,
    pub url: String,
    pub sha1: Option<String>,
    pub required_version_ids: Vec<String>,
    pub required_project_ids: Vec<String>,
}

#[async_trait]
pub trait VersionSource: Send + Sync {
    async fn fetch_version(&self, version_id: &str) -> Result<RemoteVersion, LauncherError>;
    async fn latest_for_project(
        &self,
        project_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<String, LauncherError>;
}

/// Walk required dependencies. Cycles and repeated projects are visited once.
pub async fn resolve_required(
    source: &dyn VersionSource,
    root_version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ResolvedFile>, LauncherError> {
    let mut queue = VecDeque::from([root_version_id.to_string()]);
    let mut seen_versions = HashSet::new();
    let mut seen_projects = HashSet::new();
    let mut files = Vec::new();
    let mut guard = 0u32;
    while let Some(version_id) = queue.pop_front() {
        guard += 1;
        if guard > 128 {
            return Err(LauncherError::Message(
                "dependency tree exceeded 128 nodes".into(),
            ));
        }
        if !seen_versions.insert(version_id.clone()) {
            continue;
        }
        let version = match source.fetch_version(&version_id).await {
            Ok(version) => version,
            Err(err) if version_id == root_version_id => return Err(err),
            Err(_) => continue,
        };
        if !seen_projects.insert(version.project_id.clone()) {
            continue;
        }
        files.push(ResolvedFile {
            key: version.id.clone(),
            project_id: version.project_id.clone(),
            file_name: version.file_name.clone(),
            url: version.url.clone(),
            sha1: version.sha1.clone(),
        });
        for dep_version in version.required_version_ids {
            queue.push_back(dep_version);
        }
        for project_id in version.required_project_ids {
            match source
                .latest_for_project(&project_id, game_version, loader)
                .await
            {
                Ok(id) => queue.push_back(id),
                Err(_) => continue,
            }
        }
    }
    Ok(disambiguate_filenames(files))
}

fn disambiguate_filenames(mut files: Vec<ResolvedFile>) -> Vec<ResolvedFile> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for file in &files {
        *counts.entry(file.file_name.clone()).or_default() += 1;
    }
    for file in &mut files {
        if counts.get(&file.file_name).copied().unwrap_or(0) > 1 {
            file.file_name = format!("{}-{}", file.project_id, file.file_name);
        }
    }
    files
}

pub struct ModrinthApi {
    http: HttpPool,
    cache: MetadataCache,
}

impl ModrinthApi {
    pub fn new(http: HttpPool, cache: MetadataCache) -> Self {
        Self { http, cache }
    }
}

#[async_trait]
impl VersionSource for ModrinthApi {
    async fn fetch_version(&self, version_id: &str) -> Result<RemoteVersion, LauncherError> {
        let url = format!("{MODRINTH_API}/version/{version_id}");
        let body = self
            .http
            .cached_text(
                &self.cache,
                &format!("modrinth-version:{version_id}"),
                &url,
                &[],
            )
            .await?;
        let parsed: MrVersion =
            serde_json::from_str(&body).map_err(|err| LauncherError::Provider {
                provider: "modrinth".into(),
                status: 200,
                body: format!("unreadable version {version_id}: {err}"),
            })?;
        remote_from_mr(parsed)
    }

    async fn latest_for_project(
        &self,
        project_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<String, LauncherError> {
        let game = serde_json::to_string(&vec![game_version])
            .map_err(|err| LauncherError::Message(err.to_string()))?;
        let mut url = format!(
            "{MODRINTH_API}/project/{project_id}/version?game_versions={}",
            urlencoding::encode(&game)
        );
        if !loader.is_empty() && loader != "vanilla" {
            let loaders = serde_json::to_string(&vec![loader])
                .map_err(|err| LauncherError::Message(err.to_string()))?;
            url.push_str("&loaders=");
            url.push_str(&urlencoding::encode(&loaders));
        }
        let body = self
            .http
            .cached_text(&self.cache, &format!("modrinth-project:{url}"), &url, &[])
            .await?;
        let versions: Vec<MrVersion> =
            serde_json::from_str(&body).map_err(|err| LauncherError::Provider {
                provider: "modrinth".into(),
                status: 200,
                body: format!("unreadable versions for {project_id}: {err}"),
            })?;
        versions
            .into_iter()
            .next()
            .map(|version| version.id)
            .ok_or_else(|| {
                LauncherError::Message(format!(
                    "no {loader} build of {project_id} for Minecraft {game_version}"
                ))
            })
    }
}

fn remote_from_mr(version: MrVersion) -> Result<RemoteVersion, LauncherError> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| LauncherError::MissingHash {
            url: format!("modrinth version {}", version.id),
        })?;
    let mut required_version_ids = Vec::new();
    let mut required_project_ids = Vec::new();
    for dep in version.dependencies {
        if dep.dependency_type != "required" {
            continue;
        }
        if let Some(version_id) = dep.version_id.filter(|value| !value.is_empty()) {
            required_version_ids.push(version_id);
        } else if let Some(project_id) = dep.project_id.filter(|value| !value.is_empty()) {
            required_project_ids.push(project_id);
        }
    }
    Ok(RemoteVersion {
        id: version.id,
        project_id: version.project_id,
        file_name: file.filename.clone(),
        version_number: if version.version_number.trim().is_empty() {
            file.filename.clone()
        } else {
            version.version_number
        },
        url: file.url.clone(),
        sha1: file.hashes.sha1.clone(),
        required_version_ids,
        required_project_ids,
    })
}

#[derive(Debug, Deserialize)]
struct MrVersion {
    id: String,
    project_id: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    dependencies: Vec<MrDependency>,
    #[serde(default)]
    files: Vec<MrFile>,
}

#[derive(Debug, Deserialize)]
struct MrDependency {
    version_id: Option<String>,
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: MrHashes,
}

#[derive(Debug, Default, Deserialize)]
struct MrHashes {
    sha1: Option<String>,
}

pub fn declared_sha1(hex_value: Option<String>) -> Option<ContentHash> {
    hex_value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .map(ContentHash::sha1)
}

#[derive(Debug, Deserialize)]
struct ModrinthSearch {
    #[serde(default)]
    hits: Vec<ModrinthHit>,
    #[serde(default)]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct ModrinthHit {
    project_id: String,
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    client_side: Option<String>,
    #[serde(default)]
    server_side: Option<String>,
    icon_url: Option<String>,
    #[serde(default)]
    author: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CfSearch {
    #[serde(default)]
    data: Vec<CfMod>,
    #[serde(default)]
    pagination: Option<CfPagination>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfPagination {
    #[serde(default)]
    total_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u32,
    slug: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    thumbs_up_count: u64,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    logo: Option<CfLogo>,
    #[serde(default)]
    latest_files_indexes: Vec<CfFileIndex>,
}

#[derive(Debug, Deserialize)]
struct CfAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    thumbnail_url: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileIndex {
    game_version: Option<String>,
    mod_loader: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MapSource {
        versions: Mutex<HashMap<String, RemoteVersion>>,
        latest: HashMap<String, String>,
    }

    #[async_trait]
    impl VersionSource for MapSource {
        async fn fetch_version(&self, version_id: &str) -> Result<RemoteVersion, LauncherError> {
            self.versions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(version_id)
                .cloned()
                .ok_or_else(|| LauncherError::Message(format!("missing {version_id}")))
        }

        async fn latest_for_project(
            &self,
            project_id: &str,
            _game_version: &str,
            _loader: &str,
        ) -> Result<String, LauncherError> {
            self.latest
                .get(project_id)
                .cloned()
                .ok_or_else(|| LauncherError::Message(format!("no build for {project_id}")))
        }
    }

    fn version(id: &str, project: &str, versions: &[&str], projects: &[&str]) -> RemoteVersion {
        RemoteVersion {
            id: id.into(),
            project_id: project.into(),
            file_name: format!("{project}.jar"),
            version_number: id.into(),
            url: format!("https://example.invalid/{id}.jar"),
            sha1: Some("abc".into()),
            required_version_ids: versions.iter().map(|v| (*v).to_string()).collect(),
            required_project_ids: projects.iter().map(|v| (*v).to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn resolver_follows_required_edges_once() {
        let mut versions = HashMap::new();
        versions.insert("a".into(), version("a", "alpha", &[], &["beta"]));
        versions.insert("b".into(), version("b", "beta", &["c"], &[]));
        versions.insert("c".into(), version("c", "gamma", &["a"], &[]));
        let source = MapSource {
            versions: Mutex::new(versions),
            latest: HashMap::from([("beta".into(), "b".into())]),
        };
        let files = resolve_required(&source, "a", "1.21.1", "fabric")
            .await
            .unwrap();
        let keys: Vec<_> = files.iter().map(|file| file.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn modrinth_url_encodes_facets() {
        let query = CatalogQuery {
            text: "sodium".to_string(),
            game_version: Some("1.21.1".into()),
            loader: Some("fabric".into()),
            project_type: "mod".into(),
            limit: 20,
            offset: 0,
            show_both: false,
            source: None,
            duplicate_pref: None,
            index: None,
        };
        let url = modrinth_search_url(&query).unwrap();
        assert!(url.starts_with("https://api.modrinth.com/v2/search?"));
        assert!(url.contains("categories"));
        assert!(url.contains("fabric"));
    }

    #[test]
    fn curseforge_url_uses_json_filters() {
        let query = CatalogQuery {
            text: "jei".to_string(),
            game_version: Some("1.21.1".into()),
            loader: Some("fabric".into()),
            project_type: "mod".into(),
            limit: 20,
            offset: 0,
            show_both: false,
            source: None,
            duplicate_pref: None,
            index: Some("newest".into()),
        };
        let url = curseforge_search_url(&query);
        assert!(url.contains("gameVersions="));
        assert!(url.contains("modLoaderTypes="));
        assert!(!url.contains("&gameVersion="));
        assert!(!url.contains("&modLoaderType="));
        assert!(url.contains("sortField=11"));
    }
}
