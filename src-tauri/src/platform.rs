//! Platform detection and validation for multi-platform game archives.

const PLATFORMS: &[&str] = &[
    "windows",
    "linux",
    "windows_linux",
    "mac",
    "android",
    "unknown",
];

pub fn normalize_platform(value: &str) -> Option<String> {
    let lower = value.trim().to_lowercase().replace(' ', "");
    let canonical = match lower.as_str() {
        "windows&linux" | "win&linux" | "win+linux" | "windows+linux" | "win_linux"
        | "winlin" | "win/linux" | "windows/linux" | "pc" => "windows_linux".to_string(),
        other if PLATFORMS.contains(&other) => other.to_string(),
        _ => lower,
    };
    if PLATFORMS.contains(&canonical.as_str()) {
        Some(canonical)
    } else {
        None
    }
}

fn filename_has_pc_hint(lower: &str) -> bool {
    if lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|t| t == "pc")
    {
        return true;
    }
    lower.contains("_pc")
        || lower.contains("-pc")
        || lower.contains(".pc.")
        || lower.ends_with("_pc")
        || lower.ends_with("-pc")
}

fn filename_has_windows_hint(lower: &str) -> bool {
    lower.contains("win64")
        || lower.contains("win32")
        || lower.contains("windows")
        || lower.contains("_win")
        || lower.contains("-win")
        || lower.contains("winlin")
        || lower.contains("win_lin")
        || lower.contains("win-lin")
}

fn filename_has_linux_hint(lower: &str) -> bool {
    lower.contains("linux")
        || lower.contains("_lin")
        || lower.contains("-lin")
        || lower.contains("winlin")
        || lower.contains("win_lin")
        || lower.contains("win-lin")
}

pub fn detect_platform_from_filename(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.contains("android")
        || lower.ends_with(".apk")
        || lower.contains("_apk")
        || lower.contains("-apk")
    {
        return "android";
    }
    if filename_has_pc_hint(&lower)
        || (filename_has_windows_hint(&lower) && filename_has_linux_hint(&lower))
    {
        return "windows_linux";
    }
    if lower.contains("macos")
        || lower.contains("_mac")
        || lower.contains("-mac")
        || lower.ends_with(".dmg")
    {
        return "mac";
    }
    if filename_has_linux_hint(&lower) {
        return "linux";
    }
    if filename_has_windows_hint(&lower) {
        return "windows";
    }
    "unknown"
}

pub fn platform_label(platform: &str) -> &str {
    match platform {
        "windows" => "Windows",
        "linux" => "Linux",
        "windows_linux" => "Windows & Linux",
        "mac" => "macOS",
        "android" => "Android",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_platform_hints() {
        assert_eq!(detect_platform_from_filename("Game_v1.0_Win64.zip"), "windows");
        assert_eq!(detect_platform_from_filename("Game-Linux.rar"), "linux");
        assert_eq!(detect_platform_from_filename("Game.apk"), "android");
        assert_eq!(
            detect_platform_from_filename("Game_Win_Lin_v1.zip"),
            "windows_linux"
        );
        assert_eq!(
            detect_platform_from_filename("Game-Windows-Linux.7z"),
            "windows_linux"
        );
        assert_eq!(
            detect_platform_from_filename("Being_a_DIK_v0.12.0_PC.zip"),
            "windows_linux"
        );
        assert_eq!(
            detect_platform_from_filename("Depraved_Awakening-PC.rar"),
            "windows_linux"
        );
    }

    #[test]
    fn normalizes_bundled_platform_aliases() {
        assert_eq!(
            normalize_platform("Windows & Linux"),
            Some("windows_linux".into())
        );
        assert_eq!(normalize_platform("win+linux"), Some("windows_linux".into()));
    }
}
