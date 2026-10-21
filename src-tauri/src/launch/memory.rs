use serde::Serialize;

use crate::error::LauncherError;

const MINIMUM_HEAP_MB: u64 = 1024;
const OS_RESERVE_MB: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapPlan {
    pub xms_mb: u64,
    pub xmx_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcProfile {
    AikarG1,
    GenerationalZgc,
}

/// `total_mb` is installed RAM. Default heap is total/4.
pub fn plan_heap(total_mb: u64, requested_xmx_mb: Option<u64>) -> Result<HeapPlan, LauncherError> {
    let usable = total_mb.saturating_sub(OS_RESERVE_MB);
    if usable < MINIMUM_HEAP_MB {
        return Err(LauncherError::InsufficientMemory {
            needed_mb: MINIMUM_HEAP_MB,
            available_mb: usable,
        });
    }
    let xmx = match requested_xmx_mb {
        Some(requested) if requested > usable => {
            return Err(LauncherError::InsufficientMemory {
                needed_mb: requested,
                available_mb: usable,
            });
        }
        Some(requested) => requested.max(512).min(usable),
        None => crate::settings::default_heap_mb()
            .min(usable)
            .max(MINIMUM_HEAP_MB),
    };
    let xms = (xmx / 2).clamp(512, xmx);
    Ok(HeapPlan {
        xms_mb: xms,
        xmx_mb: xmx,
    })
}

pub fn select_gc(java_major: u32, xmx_mb: u64) -> GcProfile {
    if java_major >= 21 && xmx_mb >= 4096 {
        GcProfile::GenerationalZgc
    } else {
        GcProfile::AikarG1
    }
}

pub fn gc_flags(profile: GcProfile) -> Vec<String> {
    match profile {
        GcProfile::GenerationalZgc => vec!["-XX:+UseZGC".into(), "-XX:+ZGenerational".into()],
        GcProfile::AikarG1 => vec![
            "-XX:+UseG1GC".into(),
            "-XX:+ParallelRefProcEnabled".into(),
            "-XX:MaxGCPauseMillis=200".into(),
            "-XX:+UnlockExperimentalVMOptions".into(),
            "-XX:+DisableExplicitGC".into(),
            "-XX:+AlwaysPreTouch".into(),
            "-XX:G1NewSizePercent=30".into(),
            "-XX:G1MaxNewSizePercent=40".into(),
            "-XX:G1HeapRegionSize=8M".into(),
            "-XX:G1ReservePercent=20".into(),
            "-XX:G1HeapWastePercent=5".into(),
            "-XX:G1MixedGCCountTarget=4".into(),
            "-XX:InitiatingHeapOccupancyPercent=15".into(),
            "-XX:G1MixedGCLiveThresholdPercent=90".into(),
            "-XX:G1RSetUpdatingPauseTimePercent=5".into(),
            "-XX:SurvivorRatio=32".into(),
            "-XX:+PerfDisableSharedMem".into(),
            "-XX:MaxTenuringThreshold=1".into(),
        ],
    }
}

pub fn memory_flags(plan: HeapPlan) -> Vec<String> {
    vec![
        format!("-Xms{}M", plan.xms_mb),
        format!("-Xmx{}M", plan.xmx_mb),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_heap_when_machine_is_tight() {
        let err = plan_heap(1200, Some(2048)).unwrap_err();
        assert_eq!(err.code(), "insufficient_memory");
    }

    #[test]
    fn auto_heap_stays_inside_usable_memory() {
        let plan = plan_heap(16 * 1024, None).unwrap();
        assert!(plan.xmx_mb <= 16 * 1024 - OS_RESERVE_MB);
        assert!(plan.xms_mb <= plan.xmx_mb);
    }

    #[test]
    fn large_modern_heap_selects_zgc() {
        assert_eq!(select_gc(21, 4096), GcProfile::GenerationalZgc);
        assert_eq!(select_gc(17, 8192), GcProfile::AikarG1);
    }
}
