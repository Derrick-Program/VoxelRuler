//! 掃描系統上常見的 Java 安裝路徑，供「使用自訂 Java」時挑選。

use std::path::{Path, PathBuf};

fn java_exe_name() -> &'static str {
    if cfg!(windows) { "javaw.exe" } else { "java" }
}

fn push_if_java(out: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.is_file() && !out.contains(&candidate) {
        out.push(candidate);
    }
}

/// 掃描 `base/<每個子目錄>/<sub>/bin/java` 形式的安裝
fn scan_children(out: &mut Vec<PathBuf>, base: &Path, sub: &str) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let mut p = entry.path();
        if !sub.is_empty() {
            p = p.join(sub);
        }
        push_if_java(out, p.join("bin").join(java_exe_name()));
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::var("USERPROFILE").ok().map(PathBuf::from);
    #[cfg(not(windows))]
    return std::env::var("HOME").ok().map(PathBuf::from);
}

/// 掃描系統常見位置的 Java 安裝，回傳 java 執行檔絕對路徑清單（已去重、排序）
pub fn scan_system_javas() -> Vec<String> {
    let mut found: Vec<PathBuf> = Vec::new();
    let exe = java_exe_name();

    // JAVA_HOME 永遠優先檢查
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        push_if_java(&mut found, PathBuf::from(java_home).join("bin").join(exe));
    }

    #[cfg(target_os = "macos")]
    {
        scan_children(
            &mut found,
            Path::new("/Library/Java/JavaVirtualMachines"),
            "Contents/Home",
        );
        if let Some(home) = home_dir() {
            scan_children(
                &mut found,
                &home.join("Library/Java/JavaVirtualMachines"),
                "Contents/Home",
            );
        }
        // Homebrew（Apple Silicon / Intel）
        for brew_opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
            if let Ok(entries) = std::fs::read_dir(brew_opt) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.contains("jdk") || name.contains("java") {
                        push_if_java(&mut found, entry.path().join("bin").join(exe));
                        push_if_java(
                            &mut found,
                            entry
                                .path()
                                .join("libexec/openjdk.jdk/Contents/Home/bin")
                                .join(exe),
                        );
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        scan_children(&mut found, Path::new("/usr/lib/jvm"), "");
        scan_children(&mut found, Path::new("/usr/java"), "");
        scan_children(&mut found, Path::new("/opt/jdk"), "");
        scan_children(&mut found, Path::new("/opt/java"), "");
        push_if_java(&mut found, PathBuf::from("/usr/bin/java"));
    }

    #[cfg(target_os = "windows")]
    {
        for vendor in [
            r"C:\Program Files\Java",
            r"C:\Program Files (x86)\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Eclipse Foundation",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Amazon Corretto",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\BellSoft",
        ] {
            scan_children(&mut found, Path::new(vendor), "");
        }
    }

    // 通用版本管理器
    if let Some(home) = home_dir() {
        scan_children(&mut found, &home.join(".sdkman/candidates/java"), "");
        scan_children(&mut found, &home.join(".asdf/installs/java"), "");
        scan_children(&mut found, &home.join(".jdks"), ""); // JetBrains
    }

    let mut list: Vec<String> = found
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    list.sort();
    list.dedup();
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_does_not_panic_and_dedups() {
        let list = scan_system_javas();
        let mut sorted = list.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(list.len(), sorted.len(), "結果應已去重");
        for p in &list {
            assert!(Path::new(p).is_file(), "{p} 應為存在的檔案");
        }
    }

    #[test]
    fn test_scan_children_missing_dir_is_noop() {
        let mut out = Vec::new();
        scan_children(&mut out, Path::new("/no/such/dir/anywhere"), "");
        assert!(out.is_empty());
    }
}
