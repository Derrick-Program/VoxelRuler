use std::path::{Path, PathBuf};

fn java_exe_name() -> &'static str {
    if cfg!(windows) { "javaw.exe" } else { "java" }
}

fn push_if_java(out: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.is_file() {
        out.push(candidate);
    }
}

fn scan_children(out: &mut Vec<PathBuf>, base: &Path, sub: &str) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let exe = java_exe_name();
    out.extend(
        entries
            .flatten()
            .map(|e| {
                let p = if sub.is_empty() {
                    e.path()
                } else {
                    e.path().join(sub)
                };
                p.join("bin").join(exe)
            })
            .filter(|p| p.is_file()),
    );
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::var("USERPROFILE").ok().map(PathBuf::from);
    #[cfg(not(windows))]
    return std::env::var("HOME").ok().map(PathBuf::from);
}

pub fn scan_system_javas() -> Vec<String> {
    let mut found: Vec<PathBuf> = Vec::new();
    let exe = java_exe_name();

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

    if let Some(home) = home_dir() {
        scan_children(&mut found, &home.join(".sdkman/candidates/java"), "");
        scan_children(&mut found, &home.join(".asdf/installs/java"), "");
        scan_children(&mut found, &home.join(".jdks"), "");
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
        assert_eq!(list.len(), sorted.len(), "Results should be deduplicated");
        for p in &list {
            assert!(Path::new(p).is_file(), "{p} should be an existing file");
        }
    }

    #[test]
    fn test_scan_children_missing_dir_is_noop() {
        let mut out = Vec::new();
        scan_children(&mut out, Path::new("/no/such/dir/anywhere"), "");
        assert!(out.is_empty());
    }
}
