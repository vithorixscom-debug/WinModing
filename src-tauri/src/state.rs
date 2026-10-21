use crate::cache::MetadataCache;
use crate::error::LauncherError;
use crate::launch::RunningGames;
use crate::net::HttpPool;
use crate::paths::AppPaths;

#[derive(Clone)]
pub struct AppState {
    pub paths: AppPaths,
    pub http: HttpPool,
    pub cache: MetadataCache,
    pub games: RunningGames,
}

impl AppState {
    pub fn open() -> Result<Self, LauncherError> {
        let paths = AppPaths::resolve()?;
        let http = HttpPool::new()?;
        let cache = MetadataCache::open(&paths.database)?;
        Ok(Self {
            paths,
            http,
            cache,
            games: RunningGames::default(),
        })
    }
}
