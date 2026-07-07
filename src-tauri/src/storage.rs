use std::path::Path;
use walkdir::WalkDir;

pub fn directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
pub struct VolumeStats {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn volume_stats(path: &Path) -> Option<VolumeStats> {
    volume_stats_impl(path)
}

#[cfg(unix)]
fn volume_stats_impl(path: &Path) -> Option<VolumeStats> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let check = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    let c_path = CString::new(check.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return None;
    }

    let block_size = stat.f_frsize as u64;
    Some(VolumeStats {
        total_bytes: stat.f_blocks as u64 * block_size,
        available_bytes: stat.f_bavail as u64 * block_size,
    })
}

#[cfg(not(unix))]
fn volume_stats_impl(_path: &Path) -> Option<VolumeStats> {
    None
}
