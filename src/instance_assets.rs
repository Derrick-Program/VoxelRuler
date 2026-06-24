//! 實例資料夾內容掃描與管理。
//!
//! 提供實例詳細視窗各分頁所需的檔案系統操作：
//! mods / resourcepacks / shaderpacks（列出、啟用/停用、刪除、加入）、
//! saves（世界清單，含 level.dat 的 LevelName 解析）、
//! servers.dat（極簡 NBT 解析）、screenshots、logs（含 .gz）、notes。

use anyhow::{Context as _, bail};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DISABLED_SUFFIX: &str = ".disabled";

/// 防止 UI 傳入的檔名跳脫目標資料夾
fn validate_file_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        bail!("Invalid filename: {name}");
    }
    Ok(())
}

fn human_size(bytes: u64) -> String {
    const KIB: u64 = 1 << 10;
    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn modified_string(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// 一般檔案分頁（mods / resourcepacks / shaderpacks）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FsEntry {
    /// 磁碟上的實際檔名（可能含 `.disabled` 後綴）
    pub file_name: String,
    /// 顯示用資訊（大小 / 修改時間）
    pub info: String,
    pub enabled: bool,
}

/// 列出資料夾中符合副檔名的項目（副檔名比對時先剝掉 `.disabled` 後綴）。
/// `allow_dirs` 為 true 時也列出子資料夾（資源包/光影包可為資料夾形式）。
pub fn list_entries(dir: &Path, exts: &[&str], allow_dirs: bool) -> Vec<FsEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<FsEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') {
                return None;
            }
            let enabled = !file_name.ends_with(DISABLED_SUFFIX);
            let logical = file_name
                .strip_suffix(DISABLED_SUFFIX)
                .unwrap_or(&file_name);
            let is_dir = path.is_dir();
            if is_dir {
                if !allow_dirs {
                    return None;
                }
            } else {
                let matched = exts
                    .iter()
                    .any(|ext| logical.to_lowercase().ends_with(&ext.to_lowercase()));
                if !matched {
                    return None;
                }
            }
            let info = if is_dir {
                format!("Folder · {}", modified_string(&path))
            } else {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                format!("{} · {}", human_size(size), modified_string(&path))
            };
            Some(FsEntry {
                file_name,
                info,
                enabled,
            })
        })
        .collect();
    out.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    out
}

/// 啟用/停用切換：在檔名尾端加上或移除 `.disabled`。回傳新檔名。
pub fn toggle_disabled(dir: &Path, file_name: &str) -> anyhow::Result<String> {
    validate_file_name(file_name)?;
    let src = dir.join(file_name);
    if !src.exists() {
        bail!("File not found: {file_name}");
    }
    let new_name = match file_name.strip_suffix(DISABLED_SUFFIX) {
        Some(stripped) => stripped.to_string(),
        None => format!("{file_name}{DISABLED_SUFFIX}"),
    };
    let dst = dir.join(&new_name);
    if dst.exists() {
        bail!("Target filename already exists: {new_name}");
    }
    std::fs::rename(&src, &dst).with_context(|| format!("Failed to rename {file_name}"))?;
    Ok(new_name)
}

/// 刪除檔案或資料夾（世界 / 截圖 / 模組共用）
pub fn delete_entry(dir: &Path, file_name: &str) -> anyhow::Result<()> {
    validate_file_name(file_name)?;
    let target = dir.join(file_name);
    if target.is_dir() {
        std::fs::remove_dir_all(&target).with_context(|| format!("Failed to delete directory {file_name}"))?;
    } else if target.exists() {
        std::fs::remove_file(&target).with_context(|| format!("Failed to delete file {file_name}"))?;
    }
    Ok(())
}

/// 將外部檔案複製進資料夾（加入模組 / 資源包 / 光影包）。回傳檔名。
pub fn add_file(dir: &Path, src: &Path) -> anyhow::Result<String> {
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .context("Source file has no filename")?;
    std::fs::create_dir_all(dir)?;
    std::fs::copy(src, dir.join(&name)).with_context(|| format!("Failed to copy {name}"))?;
    Ok(name)
}

// ─────────────────────────────────────────────────────────────────────────────
// 極簡 NBT 解析（servers.dat 未壓縮、level.dat 為 gzip）
// ─────────────────────────────────────────────────────────────────────────────

const NBT_MAX_LEN: usize = 1_000_000;

#[derive(Debug, Clone)]
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        match self {
            NbtTag::Compound(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            NbtTag::String(s) => Some(s),
            _ => None,
        }
    }
}

fn read_u8(r: &mut impl Read) -> anyhow::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

macro_rules! read_be {
    ($fn_name:ident, $ty:ty, $n:expr) => {
        fn $fn_name(r: &mut impl Read) -> anyhow::Result<$ty> {
            let mut b = [0u8; $n];
            r.read_exact(&mut b)?;
            Ok(<$ty>::from_be_bytes(b))
        }
    };
}
read_be!(read_i16, i16, 2);
read_be!(read_i32, i32, 4);
read_be!(read_i64, i64, 8);
read_be!(read_f32, f32, 4);
read_be!(read_f64, f64, 8);

fn read_nbt_string(r: &mut impl Read) -> anyhow::Result<String> {
    let len = read_i16(r)? as usize;
    if len > NBT_MAX_LEN {
        bail!("NBT string length abnormal: {len}");
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    // Minecraft 使用 modified UTF-8；一般伺服器名稱用 lossy 轉換已足夠
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn checked_len(len: i32) -> anyhow::Result<usize> {
    if len < 0 || len as usize > NBT_MAX_LEN {
        bail!("NBT length abnormal: {len}");
    }
    Ok(len as usize)
}

fn read_payload(r: &mut impl Read, type_id: u8, depth: u8) -> anyhow::Result<NbtTag> {
    if depth > 64 {
        bail!("NBT nested too deep");
    }
    Ok(match type_id {
        1 => NbtTag::Byte(read_u8(r)? as i8),
        2 => NbtTag::Short(read_i16(r)?),
        3 => NbtTag::Int(read_i32(r)?),
        4 => NbtTag::Long(read_i64(r)?),
        5 => NbtTag::Float(read_f32(r)?),
        6 => NbtTag::Double(read_f64(r)?),
        7 => {
            let len = checked_len(read_i32(r)?)?;
            let mut buf = vec![0u8; len];
            r.read_exact(&mut buf)?;
            NbtTag::ByteArray(buf)
        }
        8 => NbtTag::String(read_nbt_string(r)?),
        9 => {
            let item_type = read_u8(r)?;
            let len = checked_len(read_i32(r)?)?;
            let mut items = Vec::with_capacity(len.min(1024));
            for _ in 0..len {
                items.push(read_payload(r, item_type, depth + 1)?);
            }
            NbtTag::List(items)
        }
        10 => {
            let mut map = HashMap::new();
            loop {
                let child_type = read_u8(r)?;
                if child_type == 0 {
                    break;
                }
                let name = read_nbt_string(r)?;
                let value = read_payload(r, child_type, depth + 1)?;
                map.insert(name, value);
            }
            NbtTag::Compound(map)
        }
        11 => {
            let len = checked_len(read_i32(r)?)?;
            let mut items = Vec::with_capacity(len.min(1024));
            for _ in 0..len {
                items.push(read_i32(r)?);
            }
            NbtTag::IntArray(items)
        }
        12 => {
            let len = checked_len(read_i32(r)?)?;
            let mut items = Vec::with_capacity(len.min(1024));
            for _ in 0..len {
                items.push(read_i64(r)?);
            }
            NbtTag::LongArray(items)
        }
        other => bail!("Unknown NBT tag type: {other}"),
    })
}

/// 解析 NBT root（必須是 TAG_Compound），回傳 (root 名稱, 內容)
pub fn parse_nbt(r: &mut impl Read) -> anyhow::Result<(String, NbtTag)> {
    let type_id = read_u8(r)?;
    if type_id != 10 {
        bail!("NBT root is not Compound (type={type_id})");
    }
    let name = read_nbt_string(r)?;
    let tag = read_payload(r, 10, 0)?;
    Ok((name, tag))
}

// ─────────────────────────────────────────────────────────────────────────────
// 伺服器（servers.dat）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ServerEntry {
    pub name: String,
    pub ip: String,
}

/// 解析遊戲目錄下的 servers.dat（未壓縮 NBT）
pub fn read_servers(dat_path: &Path) -> anyhow::Result<Vec<ServerEntry>> {
    let bytes = std::fs::read(dat_path).context("Failed to read servers.dat")?;
    let (_, root) = parse_nbt(&mut bytes.as_slice())?;
    let mut out = Vec::new();
    if let Some(NbtTag::List(servers)) = root.get("servers") {
        for server in servers {
            let name = server
                .get("name")
                .and_then(|t| t.as_str())
                .unwrap_or("(Unnamed)")
                .to_string();
            let ip = server
                .get("ip")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            out.push(ServerEntry { name, ip });
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// 世界（saves/）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorldEntry {
    /// saves/ 下的資料夾名（刪除 / 開啟時使用）
    pub dir_name: String,
    /// level.dat 內的 LevelName（解析失敗時等同 dir_name）
    pub level_name: String,
    pub info: String,
}

fn read_level_name(level_dat: &Path) -> Option<String> {
    let file = std::fs::File::open(level_dat).ok()?;
    let mut gz = flate2::read::GzDecoder::new(file);
    let (_, root) = parse_nbt(&mut gz).ok()?;
    root.get("Data")?
        .get("LevelName")?
        .as_str()
        .map(|s| s.to_string())
}

pub fn list_worlds(saves_dir: &Path) -> Vec<WorldEntry> {
    let Ok(read) = std::fs::read_dir(saves_dir) else {
        return Vec::new();
    };
    let mut out: Vec<WorldEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let level_dat = path.join("level.dat");
            if !level_dat.is_file() {
                return None;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let level_name = read_level_name(&level_dat).unwrap_or_else(|| dir_name.clone());
            let info = format!("Last played: {}", modified_string(&level_dat));
            Some(WorldEntry {
                dir_name,
                level_name,
                info,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        a.level_name
            .to_lowercase()
            .cmp(&b.level_name.to_lowercase())
    });
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// 螢幕擷取畫面（screenshots/）
// ─────────────────────────────────────────────────────────────────────────────

/// 依修改時間新→舊列出截圖路徑，最多 `limit` 張（縮圖載入成本考量）
pub fn list_screenshots(dir: &Path, limit: usize) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = read
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let ext = path.extension()?.to_string_lossy().to_lowercase();
            if !matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.into_iter().take(limit).map(|(p, _)| p).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// 紀錄檔（logs/，含 .gz）
// ─────────────────────────────────────────────────────────────────────────────

pub fn list_log_files(logs_dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(logs_dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = read
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".log") || name.ends_with(".log.gz") || name.ends_with(".txt") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    // latest.log 排最前，其餘倒序（新日期在前）
    out.sort_by(|a, b| match (a.as_str(), b.as_str()) {
        ("latest.log", _) => std::cmp::Ordering::Less,
        (_, "latest.log") => std::cmp::Ordering::Greater,
        _ => b.cmp(a),
    });
    out
}

/// 讀取單一 log 檔內容（自動處理 .gz），只保留最後 `max_lines` 行
pub fn read_log_lines(
    logs_dir: &Path,
    name: &str,
    max_lines: usize,
) -> anyhow::Result<Vec<String>> {
    validate_file_name(name)?;
    let path = logs_dir.join(name);
    let content = if name.ends_with(".gz") {
        let file = std::fs::File::open(&path).with_context(|| format!("Failed to open {name}"))?;
        let mut gz = flate2::read::GzDecoder::new(file);
        let mut s = String::new();
        gz.read_to_string(&mut s)
            .with_context(|| format!("Failed to extract {name}"))?;
        s
    } else {
        std::fs::read_to_string(&path).with_context(|| format!("Failed to read {name}"))?
    };
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].to_vec())
}

// ─────────────────────────────────────────────────────────────────────────────
// 筆記（notes.txt）
// ─────────────────────────────────────────────────────────────────────────────

pub fn read_notes(instance_dir: &Path) -> String {
    std::fs::read_to_string(instance_dir.join("notes.txt")).unwrap_or_default()
}

pub fn save_notes(instance_dir: &Path, text: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(instance_dir)?;
    std::fs::write(instance_dir.join("notes.txt"), text).context("Failed to save notes")
}

// ─────────────────────────────────────────────────────────────────────────────
// 複製實例（遞迴複製資料夾）
// ─────────────────────────────────────────────────────────────────────────────

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &dst_path)?;
        }
        // symlink 一律跳過（避免循環與跳脫）
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// 手工組一份 servers.dat：root compound { servers: List[Compound{name, ip}] }
    fn build_servers_dat(servers: &[(&str, &str)]) -> Vec<u8> {
        fn put_str(buf: &mut Vec<u8>, s: &str) {
            buf.extend((s.len() as i16).to_be_bytes());
            buf.extend(s.as_bytes());
        }
        let mut b: Vec<u8> = Vec::new();
        b.push(10); // root compound
        put_str(&mut b, "");
        b.push(9); // list
        put_str(&mut b, "servers");
        b.push(10); // item type: compound
        b.extend((servers.len() as i32).to_be_bytes());
        for (name, ip) in servers {
            b.push(8);
            put_str(&mut b, "name");
            put_str(&mut b, name);
            b.push(8);
            put_str(&mut b, "ip");
            put_str(&mut b, ip);
            b.push(0); // end of compound
        }
        b.push(0); // end of root
        b
    }

    #[test]
    fn test_parse_servers_dat() {
        let dir = tempfile::tempdir().unwrap();
        let dat = dir.path().join("servers.dat");
        std::fs::write(
            &dat,
            build_servers_dat(&[("Hypixel", "mc.hypixel.net"), ("Local", "localhost:25565")]),
        )
        .unwrap();
        let servers = read_servers(&dat).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "Hypixel");
        assert_eq!(servers[0].ip, "mc.hypixel.net");
        assert_eq!(servers[1].name, "Local");
    }

    #[test]
    fn test_parse_servers_dat_empty_or_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_servers(&dir.path().join("nope.dat")).is_err());
        let dat = dir.path().join("servers.dat");
        std::fs::write(&dat, build_servers_dat(&[])).unwrap();
        assert!(read_servers(&dat).unwrap().is_empty());
    }

    #[test]
    fn test_level_name_from_gzip_nbt() {
        // root compound { Data: Compound { LevelName: "Minecraft" } }
        let mut inner: Vec<u8> = Vec::new();
        inner.push(10);
        inner.extend((0i16).to_be_bytes()); // root name ""
        inner.push(10);
        inner.extend((4i16).to_be_bytes());
        inner.extend(b"Data");
        inner.push(8);
        inner.extend((9i16).to_be_bytes());
        inner.extend(b"LevelName");
        let name = "Minecraft";
        inner.extend((name.len() as i16).to_be_bytes());
        inner.extend(name.as_bytes());
        inner.push(0); // end Data
        inner.push(0); // end root

        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves").join("world1");
        std::fs::create_dir_all(&saves).unwrap();
        let mut gz = flate2::write::GzEncoder::new(
            std::fs::File::create(saves.join("level.dat")).unwrap(),
            flate2::Compression::default(),
        );
        gz.write_all(&inner).unwrap();
        gz.finish().unwrap();

        let worlds = list_worlds(&dir.path().join("saves"));
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].dir_name, "world1");
        assert_eq!(worlds[0].level_name, "Minecraft");
    }

    #[test]
    fn test_list_and_toggle_mods() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sodium.jar"), b"x").unwrap();
        std::fs::write(dir.path().join("lithium.jar.disabled"), b"x").unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"x").unwrap();

        let entries = list_entries(dir.path(), &[".jar"], false);
        assert_eq!(entries.len(), 2);
        let lithium = entries
            .iter()
            .find(|e| e.file_name.starts_with("lithium"))
            .unwrap();
        assert!(!lithium.enabled);
        let sodium = entries
            .iter()
            .find(|e| e.file_name == "sodium.jar")
            .unwrap();
        assert!(sodium.enabled);

        // 停用 → 啟用 roundtrip
        let new_name = toggle_disabled(dir.path(), "sodium.jar").unwrap();
        assert_eq!(new_name, "sodium.jar.disabled");
        let back = toggle_disabled(dir.path(), &new_name).unwrap();
        assert_eq!(back, "sodium.jar");
        assert!(dir.path().join("sodium.jar").exists());
    }

    #[test]
    fn test_validate_file_name_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete_entry(dir.path(), "../evil").is_err());
        assert!(delete_entry(dir.path(), "a/b").is_err());
        assert!(toggle_disabled(dir.path(), "..").is_err());
    }

    #[test]
    fn test_read_log_lines_gz_and_tail() {
        let dir = tempfile::tempdir().unwrap();
        let content: String = (1..=10).map(|i| format!("line {i}\n")).collect();
        std::fs::write(dir.path().join("latest.log"), &content).unwrap();
        let mut gz = flate2::write::GzEncoder::new(
            std::fs::File::create(dir.path().join("2026-06-10-1.log.gz")).unwrap(),
            flate2::Compression::default(),
        );
        gz.write_all(content.as_bytes()).unwrap();
        gz.finish().unwrap();

        let lines = read_log_lines(dir.path(), "latest.log", 3).unwrap();
        assert_eq!(lines, vec!["line 8", "line 9", "line 10"]);
        let gz_lines = read_log_lines(dir.path(), "2026-06-10-1.log.gz", 100).unwrap();
        assert_eq!(gz_lines.len(), 10);

        let files = list_log_files(dir.path());
        assert_eq!(files[0], "latest.log");
    }

    #[test]
    fn test_copy_dir_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("mods")).unwrap();
        std::fs::write(src.join("instance.toml"), b"id = \"a\"").unwrap();
        std::fs::write(src.join("mods/x.jar"), b"jar").unwrap();

        let dst = dir.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("instance.toml").is_file());
        assert!(dst.join("mods/x.jar").is_file());
    }

    #[test]
    fn test_notes_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_notes(dir.path()), "");
        save_notes(dir.path(), "hello\nworld").unwrap();
        assert_eq!(read_notes(dir.path()), "hello\nworld");
    }
}
