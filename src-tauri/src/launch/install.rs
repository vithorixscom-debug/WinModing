use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::ContentHash;
use crate::error::LauncherError;
use crate::hashutil::verify_files_parallel;
use crate::launch::args::{
    is_native_coordinate, maven_relative_path, parse_version_document, rules_allow, AssetIndexFile,
    HostEnv, LibraryArtifact, VersionDocument,
};
use crate::net::{DownloadJob, HttpPool};
use crate::paths::AppPaths;
use crate::protocol::{asset_object_url, MINECRAFT_LIBRARIES, VERSION_MANIFEST_V2};

const DOWNLOAD_CONCURRENCY: usize = 24;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub stage: String,
    pub completed: u32,
    pub total: u32,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub version_id: String,
    pub files: u32,
    pub java_major: u32,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
}

pub async fn install_vanilla(
    http: &HttpPool,
    paths: &AppPaths,
    version_id: &str,
    mut progress: impl FnMut(InstallProgress),
) -> Result<InstallReport, LauncherError> {
    progress(stage("manifest", 0, 1, version_id));
    let manifest_text = http.get_text(VERSION_MANIFEST_V2, &[]).await?;
    let manifest: Manifest =
        serde_json::from_str(&manifest_text).map_err(|err| LauncherError::CorruptVersion {
            reason: format!("version manifest: {err}"),
        })?;
    let entry = manifest
        .versions
        .into_iter()
        .find(|item| item.id == version_id)
        .ok_or_else(|| LauncherError::CorruptVersion {
            reason: format!("version {version_id} is not in the manifest"),
        })?;
    let json_text = http.get_text(&entry.url, &[]).await?;
    let document = parse_version_document(&json_text)?;
    let version_dir = paths.versions.join(version_id);
    tokio::fs::create_dir_all(&version_dir)
        .await
        .map_err(|err| crate::error::io_at(&version_dir, err))?;
    let json_path = paths.version_json(version_id);
    tokio::fs::write(&json_path, &json_text)
        .await
        .map_err(|err| crate::error::io_at(&json_path, err))?;

    let host = HostEnv::current();
    let mut jobs = Vec::new();
    if let Some(client) = document.downloads.as_ref().map(|item| &item.client) {
        let hash = required_sha1(&client.sha1, &client.url)?;
        jobs.push(DownloadJob {
            url: client.url.clone(),
            dest: paths.version_jar(version_id),
            hash: Some(hash),
        });
    } else {
        return Err(LauncherError::CorruptVersion {
            reason: "client download is missing".into(),
        });
    }
    jobs.extend(collect_library_jobs(paths, &document, &host, false)?);

    let asset_index =
        document
            .asset_index
            .clone()
            .ok_or_else(|| LauncherError::CorruptVersion {
                reason: "assetIndex is missing".into(),
            })?;
    let index_hash = required_sha1(&asset_index.sha1, &asset_index.url)?;
    let index_path = paths
        .assets
        .join("indexes")
        .join(format!("{}.json", asset_index.id));
    jobs.push(DownloadJob {
        url: asset_index.url.clone(),
        dest: index_path.clone(),
        hash: Some(index_hash),
    });

    download_with_progress(http, jobs, "libraries", &mut progress).await?;

    let index_text = tokio::fs::read_to_string(&index_path)
        .await
        .map_err(|err| crate::error::io_at(&index_path, err))?;
    let index: AssetIndexFile =
        serde_json::from_str(&index_text).map_err(|err| LauncherError::CorruptVersion {
            reason: format!("asset index: {err}"),
        })?;
    let mut asset_jobs = Vec::new();
    for (_name, value) in index.objects {
        let hash = value
            .get("hash")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        if hash.len() < 2 {
            continue;
        }
        let Some(url) = asset_object_url(hash) else {
            continue;
        };
        let dest = paths.assets.join("objects").join(&hash[..2]).join(hash);
        asset_jobs.push(DownloadJob {
            url,
            dest,
            hash: Some(ContentHash::sha1(hash)),
        });
    }
    let asset_count = asset_jobs.len() as u32;
    download_with_progress(http, asset_jobs, "assets", &mut progress).await?;

    let java_major = document
        .java_version
        .as_ref()
        .map(|java| java.major_version)
        .unwrap_or(21);
    let _ = entry.version_type;
    Ok(InstallReport {
        version_id: version_id.to_string(),
        files: asset_count,
        java_major,
    })
}

pub fn collect_library_jobs(
    paths: &AppPaths,
    document: &VersionDocument,
    host: &HostEnv,
    missing_only: bool,
) -> Result<Vec<DownloadJob>, LauncherError> {
    let mut jobs = Vec::new();
    for library in &document.libraries {
        if !rules_allow(&library.rules, host) {
            continue;
        }
        if let Some(artifact) = &library.downloads.artifact {
            if let Some(job) = library_job(paths, &library.name, artifact)? {
                if !missing_only || !job.dest.exists() {
                    jobs.push(job);
                }
            }
        } else if let Some(job) = maven_library_job(paths, library)? {
            if !missing_only || !job.dest.exists() {
                jobs.push(job);
            }
        }
        if let Some(classifiers) = &library.downloads.classifiers {
            let key = format!("natives-{}", host.os_name);
            if let Some(value) = classifiers.get(&key) {
                if let Ok(artifact) = serde_json::from_value::<LibraryArtifact>(value.clone()) {
                    let coordinate = format!("{}:{key}", library.name);
                    if let Some(job) = library_job(paths, &coordinate, &artifact)? {
                        if !missing_only || !job.dest.exists() {
                            jobs.push(job);
                        }
                    }
                }
            }
        }
    }
    Ok(jobs)
}

pub async fn ensure_libraries(
    http: &HttpPool,
    paths: &AppPaths,
    document: &VersionDocument,
    host: &HostEnv,
) -> Result<(), LauncherError> {
    let jobs = collect_library_jobs(paths, document, host, true)?;
    if jobs.is_empty() {
        return Ok(());
    }
    download_with_progress(http, jobs, "libraries", &mut |_| {}).await
}

fn library_job(
    paths: &AppPaths,
    coordinate: &str,
    artifact: &LibraryArtifact,
) -> Result<Option<DownloadJob>, LauncherError> {
    let relative = match &artifact.path {
        Some(path) if !path.is_empty() => path.clone(),
        _ => maven_relative_path(coordinate)?,
    };
    let url = if artifact.url.starts_with("http") {
        artifact.url.clone()
    } else if !relative.is_empty() {
        format!("{MINECRAFT_LIBRARIES}/{relative}")
    } else {
        return Ok(None);
    };
    let hash = artifact
        .sha1
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ContentHash::sha1);
    Ok(Some(DownloadJob {
        url,
        dest: paths.libraries.join(relative),
        hash,
    }))
}

fn maven_library_job(
    paths: &AppPaths,
    library: &crate::launch::args::LibrarySpec,
) -> Result<Option<DownloadJob>, LauncherError> {
    let base = library
        .url
        .as_deref()
        .map(str::trim)
        .filter(|value| value.starts_with("http"))
        .map(|value| value.trim_end_matches('/').to_string());
    let Some(base) = base else {
        return Ok(None);
    };
    let relative = maven_relative_path(&library.name)?;
    Ok(Some(DownloadJob {
        url: format!("{base}/{relative}"),
        dest: paths.libraries.join(&relative),
        hash: None,
    }))
}

fn required_sha1(value: &Option<String>, url: &str) -> Result<ContentHash, LauncherError> {
    match value
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        Some(hash) => Ok(ContentHash::sha1(hash)),
        None => Err(LauncherError::MissingHash {
            url: url.to_string(),
        }),
    }
}

async fn download_with_progress(
    http: &HttpPool,
    jobs: Vec<DownloadJob>,
    stage_name: &str,
    progress: &mut impl FnMut(InstallProgress),
) -> Result<(), LauncherError> {
    let total = jobs.len() as u32;
    progress(stage(stage_name, 0, total, ""));
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(DOWNLOAD_CONCURRENCY));
    let mut tasks = Vec::with_capacity(jobs.len());
    for job in jobs {
        let permit = semaphore.clone();
        let client = http.clone();
        let label = job
            .dest
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        tasks.push(tokio::spawn(async move {
            let _ticket = permit
                .acquire_owned()
                .await
                .map_err(|err| LauncherError::Message(err.to_string()))?;
            if let Some(hash) = &job.hash {
                client.download_verified(&job.url, &job.dest, hash).await?;
            } else {
                log::warn!("no hash for {}; checking HTTPS host and file size", job.url);
                crate::protocol::assert_trusted_https(&job.url)?;
                client.download_to(&job.url, &job.dest).await?;
                let meta = tokio::fs::metadata(&job.dest)
                    .await
                    .map_err(|err| crate::error::io_at(&job.dest, err))?;
                if meta.len() == 0 {
                    return Err(LauncherError::CorruptVersion {
                        reason: format!("empty download from {}", job.url),
                    });
                }
            }
            Ok::<String, LauncherError>(label)
        }));
    }
    let mut completed = 0u32;
    for task in tasks {
        let label = match task.await {
            Ok(result) => result?,
            Err(err) => {
                return Err(LauncherError::Message(format!(
                    "download task failed: {err}"
                )))
            }
        };
        completed += 1;
        if completed % 20 == 0 || completed == total {
            progress(stage(stage_name, completed, total, &label));
        }
    }
    Ok(())
}

fn stage(name: &str, completed: u32, total: u32, file: &str) -> InstallProgress {
    InstallProgress {
        stage: name.to_string(),
        completed,
        total,
        file: file.to_string(),
    }
}

pub async fn verify_installed_hashes(
    files: Vec<(PathBuf, ContentHash)>,
) -> Result<(), LauncherError> {
    tokio::task::spawn_blocking(move || verify_files_parallel(&files))
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?
}

pub fn collect_existing_library_paths(
    paths: &AppPaths,
    document: &VersionDocument,
    host: &HostEnv,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<String>), LauncherError> {
    let mut classpath = Vec::new();
    let mut natives = Vec::new();
    let mut missing = Vec::new();
    for library in &document.libraries {
        if !rules_allow(&library.rules, host) {
            continue;
        }
        let native = is_native_coordinate(&library.name);
        if let Some(artifact) = &library.downloads.artifact {
            let relative = match &artifact.path {
                Some(path) if !path.is_empty() => path.clone(),
                _ => maven_relative_path(&library.name)?,
            };
            let full = paths.libraries.join(&relative);
            if !full.exists() {
                missing.push(relative);
                continue;
            }
            classpath.push(full.clone());
            if native {
                natives.push(full);
            }
        } else if let Ok(relative) = maven_relative_path(&library.name) {
            let full = paths.libraries.join(&relative);
            if !full.exists() {
                missing.push(relative);
            } else {
                classpath.push(full.clone());
                if native {
                    natives.push(full);
                }
            }
        }
        if let Some(classifiers) = &library.downloads.classifiers {
            let key = format!("natives-{}", host.os_name);
            if let Some(value) = classifiers.get(&key) {
                if let Ok(artifact) = serde_json::from_value::<LibraryArtifact>(value.clone()) {
                    let coordinate = format!("{}:{key}", library.name);
                    let relative = match &artifact.path {
                        Some(path) if !path.is_empty() => path.clone(),
                        _ => maven_relative_path(&coordinate).unwrap_or_default(),
                    };
                    if relative.is_empty() {
                        continue;
                    }
                    let full = paths.libraries.join(&relative);
                    if full.exists() {
                        classpath.push(full.clone());
                        natives.push(full);
                    } else {
                        missing.push(relative);
                    }
                }
            }
        }
    }
    Ok((classpath, natives, missing))
}
