#![allow(unused)]
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use std::env::consts::{ARCH, OS};
use tracing::warn;

use crate::mc_types::{
    McArgumentItem, McArgumentValue, McFeatureRule, McLibrary, McOsRule, McRule, McRuleAction,
    McRuleArch, McRuleOS, McSpecificVersionDetail,
};

/// macOS（特別是 Apple Silicon）上，過舊的 jna 5.x 會導致原生函式庫載入問題，
/// 統一升級到此版本。install 與 classpath 兩端都以此為準，確保檔案一定存在。
pub const JNA_COMPAT_VERSION: &str = "5.13.0";

/// 若該 library 在 macOS 上需要把 jna 升到 [`JNA_COMPAT_VERSION`]，
/// 回傳 artifact 名稱（`"jna"` 或 `"jna-platform"`）。
pub fn jna_needs_bump(name: &str) -> Option<&'static str> {
    let mut parts = name.split(':');
    if parts.next()? != "net.java.dev.jna" {
        return None;
    }
    let artifact = match parts.next()? {
        "jna" => "jna",
        "jna-platform" => "jna-platform",
        _ => return None,
    };
    let mut nums = parts.next()?.split('.');
    let major: u32 = nums.next()?.parse().ok()?;
    let minor: u32 = nums.next()?.parse().ok()?;
    (major == 5 && minor < 13).then_some(artifact)
}

/// jna 升級後在 libraries 目錄下的相對路徑
pub fn jna_compat_rel_path(artifact: &str) -> PathBuf {
    PathBuf::from(format!(
        "net/java/dev/jna/{artifact}/{v}/{artifact}-{v}.jar",
        v = JNA_COMPAT_VERSION
    ))
}

/// 取得目前 OS 對應的 natives classifier key。
///
/// 新版格式（約 1.19+）natives 是獨立的 artifact library，不會走到這裡；
/// 舊版格式則透過 `natives` 欄位（OS → key，可能含 `${arch}`）指定。
/// 若 JSON 缺 `natives` 欄位（例如經過正規化的測試資料），
/// 退而求其次直接在 classifiers 中猜標準命名。
pub fn native_classifier_key(lib: &McLibrary) -> Option<String> {
    let os_key = match OS {
        "windows" => "windows",
        "macos" => "osx",
        "linux" => "linux",
        _ => return None,
    };
    let arch = if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    };
    if let Some(natives) = &lib.natives {
        return Some(natives.get(os_key)?.replace("${arch}", arch));
    }
    let classifiers = lib.downloads.as_ref()?.classifiers.as_ref()?;
    [
        format!("natives-{os_key}"),
        format!("natives-{os_key}-{arch}"),
    ]
    .into_iter()
    .find(|k| classifiers.contains_key(k))
}

#[derive(Debug)]
pub struct LaunchContext {
    pub version: McSpecificVersionDetail,
    pub java_path: PathBuf,
    /// Per-instance game directory (saves, configs, resource packs, …)
    pub game_dir: PathBuf,
    /// Shared libraries directory
    pub libraries_dir: PathBuf,
    /// Shared assets directory
    pub assets_dir: PathBuf,
    /// Per-instance extracted native libraries directory
    pub natives_dir: PathBuf,
    /// Shared versions directory; JAR lives at `<versions_dir>/<id>/<id>.jar`
    pub versions_dir: PathBuf,
    pub auth_player_name: String,
    pub auth_uuid: String,
    pub auth_access_token: String,
    /// Microsoft launcher client ID (empty string for offline mode)
    pub client_id: String,
    /// Xbox Live user ID (empty string for offline mode)
    pub xuid: String,
    /// Maximum heap size passed to JVM, e.g. `"2G"`
    pub xmx: String,
    /// Initial heap size passed to JVM, e.g. `"512M"`
    pub xms: String,
}

impl LaunchContext {
    pub fn build_command(&self) -> Command {
        let classpath = self.build_classpath();
        let vars = self.build_vars(&classpath);
        let mut cmd = Command::new(&self.java_path);
        cmd.current_dir(&self.game_dir);

        if let Some(arguments) = &self.version.arguments {
            // 優先序（後加 = 高優先，dedup 保留最後一筆）：
            //   版本 JSON jvm < default_user_jvm < 明確指定的 Xmx/Xms
            let mut all_jvm: Vec<String> = collect_args(&arguments.jvm, &vars);

            if let Some(defaults) = &arguments.default_user_jvm {
                all_jvm.extend(collect_args(defaults, &vars));
            }

            all_jvm.push(format!("-Xmx{}", self.xmx));
            all_jvm.push(format!("-Xms{}", self.xms));

            // 去除互斥 flag 的衝突，保留最後（最高優先）那一筆
            let mut all_jvm = dedup_jvm_args(all_jvm);

            // compat args 插到 -cp 之前
            let mut compat = self.java_compat_args();
            if cfg!(target_os = "macos") {
                let natives_str = self.natives_dir.to_string_lossy().into_owned();
                compat.push(format!("-Djna.tmpdir={}", natives_str));
                compat.push(format!(
                    "-Dorg.lwjgl.system.SharedLibraryExtractPath={}",
                    natives_str
                ));
                compat.push(format!("-Dio.netty.native.workdir={}", natives_str));
            }
            if !compat.is_empty() {
                let pos = all_jvm
                    .iter()
                    .position(|a| a == "-cp")
                    .unwrap_or(all_jvm.len());
                for (i, arg) in compat.into_iter().enumerate() {
                    all_jvm.insert(pos + i, arg);
                }
            }

            cmd.args(all_jvm);
            cmd.arg(&self.version.main_class);
            cmd.args(collect_args(&arguments.game, &vars));
        } else {
            // 舊版格式（無 arguments 欄位）：直接加，不會有重複問題
            cmd.arg(format!("-Xmx{}", self.xmx));
            cmd.arg(format!("-Xms{}", self.xms));
            cmd.arg(format!(
                "-Djava.library.path={}",
                self.natives_dir.display()
            ));
            cmd.arg("-cp");
            cmd.arg(classpath);
            cmd.arg(&self.version.main_class);
            if let Some(mc_args) = &self.version.minecraft_arguments {
                cmd.args(
                    mc_args
                        .split_whitespace()
                        .map(|p| resolve_argument(p, &vars)),
                );
            }
        }

        cmd
    }

    /// classpath 上所有 JAR 的絕對路徑（最後一項為版本 JAR）
    pub fn classpath_paths(&self) -> Vec<PathBuf> {
        let mut parts: Vec<PathBuf> = Vec::new();

        for lib in &self.version.libraries {
            if let Some(rules) = &lib.rules
                && !evaluate_rules(rules)
            {
                continue;
            }

            // macOS：過舊 jna 一律改指向相容版本（install 端會下載對應檔案）
            if cfg!(target_os = "macos")
                && let Some(artifact) = jna_needs_bump(&lib.name)
            {
                parts.push(self.libraries_dir.join(jna_compat_rel_path(artifact)));
                continue;
            }

            let rel: Option<PathBuf> = match &lib.downloads {
                Some(d) => match &d.artifact {
                    Some(a) => a
                        .path
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| maven_coord_to_path(&lib.name)),
                    // 只有 classifiers（natives-only）的 lib：jar 走解壓流程，不上 classpath
                    None => None,
                },
                // 無 downloads 資訊（如第三方 loader 的 lib）：以 maven 座標推路徑
                None => maven_coord_to_path(&lib.name),
            };
            if let Some(r) = rel {
                parts.push(self.libraries_dir.join(r));
            }
        }

        parts.push(
            self.versions_dir
                .join(&self.version.id)
                .join(format!("{}.jar", self.version.id)),
        );

        parts
    }

    fn build_classpath(&self) -> String {
        let sep = if cfg!(windows) { ";" } else { ":" };
        self.classpath_paths()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(sep)
    }

    /// 啟動前檢查：回傳 classpath 上實際不存在的檔案清單
    pub fn missing_classpath_files(&self) -> Vec<PathBuf> {
        self.classpath_paths()
            .into_iter()
            .filter(|p| !p.exists())
            .collect()
    }

    fn java_compat_args(&self) -> Vec<String> {
        let major = self
            .version
            .java_version
            .as_ref()
            .map_or(8, |jv| jv.major_version);

        let mut args: Vec<String> = Vec::new();
        if major >= 17 {
            args.push("--add-modules=jdk.incubator.vector".into());
            args.push("--enable-native-access=ALL-UNNAMED".into());
        }
        if major >= 21 {
            args.push("--sun-misc-unsafe-memory-access=allow".into());
        }
        args
    }

    fn build_vars(&self, classpath: &str) -> HashMap<&'static str, String> {
        let sep = if cfg!(windows) { ";" } else { ":" };
        let mut m: HashMap<&'static str, String> = HashMap::new();
        m.insert("auth_player_name", self.auth_player_name.clone());
        m.insert("auth_uuid", self.auth_uuid.clone());
        m.insert("auth_access_token", self.auth_access_token.clone());
        m.insert("auth_session", self.auth_access_token.clone());
        m.insert("clientid", self.client_id.clone());
        m.insert("auth_xuid", self.xuid.clone());
        m.insert("user_type", "msa".into());
        m.insert("version_name", self.version.id.clone());
        m.insert(
            "game_directory",
            self.game_dir.to_string_lossy().into_owned(),
        );
        m.insert(
            "assets_root",
            self.assets_dir.to_string_lossy().into_owned(),
        );
        m.insert(
            "assets_index_name",
            self.version
                .asset_index
                .as_ref()
                .map_or_else(|| "legacy".into(), |a| a.id.clone()),
        );
        m.insert(
            "game_assets",
            self.assets_dir.to_string_lossy().into_owned(),
        );
        m.insert("version_type", self.version.r#type.clone());
        m.insert(
            "natives_directory",
            self.natives_dir.to_string_lossy().into_owned(),
        );
        m.insert("launcher_name", "VoxelRuler".into());
        m.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
        m.insert("classpath", classpath.to_owned());
        m.insert(
            "library_directory",
            self.libraries_dir.to_string_lossy().into_owned(),
        );
        m.insert("classpath_separator", sep.into());
        m
    }
}

/// 去除 JVM 參數中的互斥衝突，對每種「只能有一個」的 flag 保留最後（優先序最高）那筆。
///
/// 規則：
/// - GC 選擇器（`-XX:+UseZGC` 等）彼此互斥，整組只保留最後一個
/// - 前綴唯一型（`-Xmx`、`-Xms`、`-Xss`、`-Xmn`）每種前綴只保留最後一個
/// - 其餘 flag 全部保留
fn dedup_jvm_args(args: Vec<String>) -> Vec<String> {
    const GC_FLAGS: &[&str] = &[
        "-XX:+UseG1GC",
        "-XX:+UseZGC",
        "-XX:+UseShenandoahGC",
        "-XX:+UseParallelGC",
        "-XX:+UseSerialGC",
        "-XX:+UseConcMarkSweepGC",
        "-XX:+UseEpsilonGC",
    ];
    const UNIQUE_PREFIXES: &[&str] = &["-Xmx", "-Xms", "-Xss", "-Xmn"];

    // Pass 1：找出每種互斥類型的「最後出現位置」
    let mut last_gc: Option<usize> = None;
    let mut last_prefix: Vec<Option<usize>> = vec![None; UNIQUE_PREFIXES.len()];

    for (i, arg) in args.iter().enumerate() {
        if GC_FLAGS.contains(&arg.as_str()) {
            last_gc = Some(i);
        }
        for (pi, prefix) in UNIQUE_PREFIXES.iter().enumerate() {
            if arg.starts_with(prefix) {
                last_prefix[pi] = Some(i);
            }
        }
    }

    // Pass 2：過濾，重複的非最後那筆記錄 warning 並移除
    args.into_iter()
        .enumerate()
        .filter_map(|(i, arg)| {
            if GC_FLAGS.contains(&arg.as_str()) {
                if Some(i) != last_gc {
                    warn!("移除衝突 GC 參數: {arg}");
                    return None;
                }
                return Some(arg);
            }
            for (pi, prefix) in UNIQUE_PREFIXES.iter().enumerate() {
                if arg.starts_with(prefix) {
                    if Some(i) != last_prefix[pi] {
                        warn!("移除重複 JVM 參數: {arg}");
                        return None;
                    }
                    return Some(arg);
                }
            }
            Some(arg)
        })
        .collect()
}

pub(crate) fn evaluate_rules(rules: &[McRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().is_none_or(os_rule_matches);
        let feat_ok = rule.features.as_ref().is_none_or(feature_rule_matches);
        if os_ok && feat_ok {
            allowed = rule.action == McRuleAction::Allow;
        }
    }
    allowed
}

fn os_rule_matches(os: &McOsRule) -> bool {
    if let Some(name) = &os.name {
        let ok = match name {
            McRuleOS::Windows => OS == "windows",
            McRuleOS::Osx => OS == "macos",
            McRuleOS::Linux => OS == "linux",
        };
        if !ok {
            return false;
        }
    }
    if let Some(arch) = &os.arch {
        let ok = match arch {
            McRuleArch::X86 => ARCH == "x86",
            // McRuleArch::X64 => ARCH == "x86_64",
            // McRuleArch::Arm64 => ARCH == "aarch64",
        };
        if !ok {
            return false;
        }
    }
    if let Some(pattern) = &os.version {
        let ver = get_os_version();
        let matches = Regex::new(pattern)
            .map(|re| re.is_match(ver))
            .unwrap_or(false);
        if !matches {
            return false;
        }
    }
    true
}

fn get_os_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(detect_os_version)
}

#[cfg(target_os = "macos")]
fn detect_os_version() -> String {
    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn detect_os_version() -> String {
    Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn detect_os_version() -> String {
    Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn detect_os_version() -> String {
    String::new()
}

fn feature_rule_matches(feat: &McFeatureRule) -> bool {
    feat.is_demo_user != Some(true)
        && feat.has_custom_resolution != Some(true)
        && feat.has_quick_plays_support != Some(true)
        && feat.is_quick_play_singleplayer != Some(true)
        && feat.is_quick_play_multiplayer != Some(true)
        && feat.is_quick_play_realms != Some(true)
}

fn collect_args(items: &[McArgumentItem], vars: &HashMap<&'static str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        match item {
            McArgumentItem::Simple(s) => out.push(resolve_argument(s, vars)),
            McArgumentItem::Conditional(cond) if evaluate_rules(&cond.rules) => match &cond.value {
                McArgumentValue::Single(s) => out.push(resolve_argument(s, vars)),
                McArgumentValue::Many(args) => {
                    out.extend(args.iter().map(|a| resolve_argument(a, vars)));
                }
            },
            _ => {}
        }
    }
    out
}

pub fn maven_coord_to_path(coord: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let mut path = PathBuf::new();
    for component in parts[0].split('.') {
        path.push(component);
    }
    let artifact = parts[1];
    let version = parts[2];
    path.push(artifact);
    path.push(version);
    let filename = match parts.get(3) {
        Some(cls) => format!("{artifact}-{version}-{cls}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    path.push(filename);
    Some(path)
}

pub fn get_mojang_os_arch() -> &'static str {
    match (OS, ARCH) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",

        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",

        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",

        _ => {
            warn!(os = OS, arch = ARCH, "未知的系統或架構組合");
            "unknown"
        }
    }
}

fn get_macro_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").unwrap())
}

pub fn resolve_argument<K>(arg: &str, vars: &HashMap<K, String>) -> String
where
    K: std::borrow::Borrow<str> + std::hash::Hash + Eq,
{
    let re = get_macro_regex();
    re.replace_all(arg, |caps: &regex::Captures| {
        let key: &str = &caps[1];
        vars.get(key).cloned().unwrap_or_default()
    })
    .into_owned()
}

mod test {
    use super::*;
    use crate::mc_types::{McJavaAll, McSpecificVersionDetail};
    #[allow(unused_imports)]
    use std::collections::HashMap;

    fn cmd_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn load_version(path: &str) -> McSpecificVersionDetail {
        let data = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("找不到 {path}"));
        serde_json::from_str(&data).unwrap_or_else(|_| panic!("解析 {path} 失敗"))
    }

    #[test]
    fn test_parse_var_args() {
        let data = r#"${auth_player_name}"#;
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", "TestPlayer".into());
        let ans = resolve_argument(data, &vars);
        assert_eq!(ans, "TestPlayer");
    }

    #[tokio::test]
    async fn test_parse_mc_specific_version_detail() {
        let v = load_version("data/26.1.2.json");
        println!("完整結構體: {:#?}", v.arguments.unwrap().jvm);
    }

    #[tokio::test]
    async fn test_parse_java_all() {
        let data = tokio::fs::read_to_string("data/java-all.json")
            .await
            .expect("can't read file");
        let java_all: McJavaAll = serde_json::from_str(&data).expect("解析失敗");
        println!("解析成功！");
        println!("完整結構體: {:#?}", java_all);
        let os_arch = get_mojang_os_arch();
        println!("當前系統架構對應的 Mojang 字串: {}", os_arch);
        let mac_java = java_all.get(os_arch).unwrap();
        println!("{} Java 版本: {:#?}", os_arch, mac_java);
    }

    #[tokio::test]
    async fn test_java_parse() {
        let os_arch = get_mojang_os_arch();
        println!("當前系統架構對應的 Mojang 字串: {}", os_arch);
        let data = tokio::fs::read_to_string("data/java-all.json")
            .await
            .expect("can't read file");
        let java_all: McJavaAll = serde_json::from_str(&data).expect("解析失敗");
        let java_version = java_all
            .get(os_arch)
            .unwrap()
            .get("java-runtime-delta")
            .unwrap();
        let java_manifest = &java_version.first().unwrap().manifest;
        println!("Minecraft Java Manifest: {:#?}", java_manifest);
        let url = java_manifest.url.clone();
        println!("Minecraft Java 下載 URL: {}", url);
        let response: crate::mc_types::McJavaManifest = reqwest::get(&url)
            .await
            .expect("下載失敗")
            .error_for_status()
            .expect("HTTP 錯誤")
            .json()
            .await
            .expect("解析 JSON 失敗");
        println!("Minecraft Java Manifest 內容: {:#?}", response);
    }

    #[test]
    fn test_get_mojang_os_arch() {
        let os_arch = get_mojang_os_arch();
        println!("當前系統架構對應的 Mojang 字串: {}", os_arch);
    }

    fn make_ctx(version: McSpecificVersionDetail) -> LaunchContext {
        LaunchContext {
            version,
            java_path: PathBuf::from("/usr/bin/java"),
            game_dir: PathBuf::from("/game"),
            libraries_dir: PathBuf::from("/libs"),
            assets_dir: PathBuf::from("/assets"),
            natives_dir: PathBuf::from("/natives"),
            versions_dir: PathBuf::from("/versions"),
            auth_player_name: "Steve".into(),
            auth_uuid: "uuid-1234".into(),
            auth_access_token: "token-abcd".into(),
            client_id: "".into(),
            xuid: "".into(),
            xmx: "2G".into(),
            xms: "512M".into(),
        }
    }

    fn make_ctx_with_java(version: McSpecificVersionDetail, major_version: i32) -> LaunchContext {
        let mut ctx = make_ctx(version);
        if let Some(ref mut jv) = ctx.version.java_version {
            jv.major_version = major_version;
        }
        ctx
    }

    #[test]
    fn test_java21_compat_args_injected_before_cp() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 21).build_command();
        let args = cmd_args(&cmd);

        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        assert!(
            args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "缺少 incubator.vector"
        );
        assert!(
            args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "缺少 native-access"
        );
        assert!(
            args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "缺少 unsafe-memory-access"
        );

        let native_pos = args
            .iter()
            .position(|a| a == "--enable-native-access=ALL-UNNAMED")
            .unwrap();
        let unsafe_pos = args
            .iter()
            .position(|a| a == "--sun-misc-unsafe-memory-access=allow")
            .unwrap();
        assert!(native_pos < cp_pos, "--enable-native-access 應在 -cp 之前");
        assert!(
            unsafe_pos < cp_pos,
            "--sun-misc-unsafe-memory-access 應在 -cp 之前"
        );
    }

    #[test]
    fn test_java17_compat_args_no_unsafe_memory_access() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 17).build_command();
        let args = cmd_args(&cmd);

        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        assert!(
            args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Java 17 應有 incubator.vector"
        );
        assert!(
            args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Java 17 應有 native-access"
        );
        assert!(
            !args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "Java 17 不應有 unsafe-memory-access（需要 >= 21）"
        );

        let native_pos = args
            .iter()
            .position(|a| a == "--enable-native-access=ALL-UNNAMED")
            .unwrap();
        assert!(native_pos < cp_pos, "--enable-native-access 應在 -cp 之前");
    }

    #[test]
    fn test_old_java_no_compat_args() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 8).build_command();
        let args = cmd_args(&cmd);

        assert!(
            !args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Java 8 不應有 compat args"
        );
        assert!(
            !args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Java 8 不應有 compat args"
        );
        assert!(
            !args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "Java 8 不應有 compat args"
        );
    }

    #[test]
    fn test_build_command_from_1_21_json() {
        let cmd = make_ctx(load_version("data/1.21.json")).build_command();
        let args = cmd_args(&cmd);

        assert!(args.contains(&"-Xmx2G".into()), "缺少 -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "缺少 -Xms");
        assert!(
            args.contains(&"-Djava.library.path=/natives".into()),
            "natives_directory 未替換"
        );
        assert!(
            args.contains(&"-Djna.tmpdir=/natives".into()),
            "natives_directory 未替換（jna）"
        );
        assert!(args.contains(&"-cp".into()), "缺少 -cp");

        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        let classpath = &args[cp_pos + 1];
        assert!(
            classpath.ends_with("1.21/1.21.jar"),
            "classpath 應以版本 JAR 結尾"
        );

        #[cfg(target_os = "macos")]
        {
            assert!(
                classpath.contains("java-objc-bridge"),
                "macOS classpath 應包含 java-objc-bridge"
            );
            assert!(
                !classpath.contains("natives-linux"),
                "macOS classpath 不應有 linux natives"
            );
            assert!(
                !classpath.contains("natives-windows"),
                "macOS classpath 不應有 windows natives"
            );
        }
        #[cfg(target_os = "linux")]
        {
            assert!(
                classpath.contains("natives-linux"),
                "Linux classpath 應包含 linux natives"
            );
            assert!(
                !classpath.contains("natives-macos"),
                "Linux classpath 不應有 macos natives"
            );
        }
        #[cfg(target_os = "windows")]
        {
            assert!(
                classpath.contains("natives-windows"),
                "Windows classpath 應包含 windows natives"
            );
            assert!(
                !classpath.contains("natives-linux"),
                "Windows classpath 不應有 linux natives"
            );
        }

        assert!(
            args.contains(&"net.minecraft.client.main.Main".into()),
            "缺少 main class"
        );

        assert!(args.contains(&"--username".into()));
        assert!(args.contains(&"Steve".into()), "auth_player_name 未替換");
        assert!(args.contains(&"1.21".into()), "version_name 未替換");
        assert!(args.contains(&"--gameDir".into()));
        assert!(args.contains(&"/game".into()), "game_directory 未替換");
        assert!(args.contains(&"msa".into()), "user_type 應為 msa");

        assert!(
            !args.contains(&"--demo".into()),
            "--demo 不應出現（非 demo 模式）"
        );
        assert!(
            !args.contains(&"--width".into()),
            "--width 不應出現（無自訂解析度）"
        );
        assert!(
            !args.contains(&"--quickPlayPath".into()),
            "--quickPlayPath 不應出現"
        );

        #[cfg(target_os = "macos")]
        assert!(
            args.contains(&"-XstartOnFirstThread".into()),
            "macOS 應有 -XstartOnFirstThread"
        );
        #[cfg(not(target_os = "macos"))]
        assert!(
            !args.contains(&"-XstartOnFirstThread".into()),
            "非 macOS 不應有 -XstartOnFirstThread"
        );
        #[cfg(not(target_os = "windows"))]
        assert!(
            !args.iter().any(|a| a.contains("HeapDumpPath")),
            "非 Windows 不應有 HeapDumpPath"
        );
    }

    #[test]
    fn test_build_command_from_1_12_2_json() {
        let cmd = make_ctx(load_version("data/1.12.2.json")).build_command();
        let args = cmd_args(&cmd);

        assert!(args.contains(&"-Xmx2G".into()), "缺少 -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "缺少 -Xms");

        assert!(
            args.contains(&"-Djava.library.path=/natives".into()),
            "缺少 natives_directory"
        );
        assert!(args.contains(&"-cp".into()), "缺少 -cp");

        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        let classpath = &args[cp_pos + 1];
        assert!(
            classpath.ends_with("1.12.2/1.12.2.jar"),
            "classpath 應以版本 JAR 結尾"
        );

        #[cfg(target_os = "macos")]
        {
            assert!(
                !classpath.contains("lwjgl-2.9.4"),
                "macOS: lwjgl 2.9.4 應被 disallow 排除"
            );
            assert!(
                classpath.contains("lwjgl-2.9.2"),
                "macOS: lwjgl 2.9.2 應被 allow 包含"
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(
                classpath.contains("lwjgl-2.9.4"),
                "非 macOS: lwjgl 2.9.4 應被包含"
            );
            assert!(
                !classpath.contains("lwjgl-2.9.2"),
                "非 macOS: lwjgl 2.9.2（macOS 專用）應被排除"
            );
        }

        assert!(
            args.contains(&"net.minecraft.client.main.Main".into()),
            "缺少 main class"
        );

        assert!(args.contains(&"--username".into()));
        assert!(args.contains(&"Steve".into()), "auth_player_name 未替換");
        assert!(args.contains(&"1.12.2".into()), "version_name 未替換");
        assert!(args.contains(&"--gameDir".into()));
        assert!(args.contains(&"/game".into()), "game_directory 未替換");
        assert!(args.contains(&"--userType".into()));
        assert!(args.contains(&"msa".into()), "user_type 應為 msa");
        assert!(args.contains(&"--uuid".into()));
        assert!(args.contains(&"uuid-1234".into()), "auth_uuid 未替換");
    }

    #[test]
    fn test_jna_needs_bump() {
        assert_eq!(jna_needs_bump("net.java.dev.jna:jna:5.10.0"), Some("jna"));
        assert_eq!(
            jna_needs_bump("net.java.dev.jna:jna-platform:5.8.0"),
            Some("jna-platform")
        );
        // 已是相容版本以上 → 不升
        assert_eq!(jna_needs_bump("net.java.dev.jna:jna:5.13.0"), None);
        assert_eq!(jna_needs_bump("net.java.dev.jna:jna:5.14.0"), None);
        // 3.x / 4.x 太舊，API 不相容，不做替換
        assert_eq!(jna_needs_bump("net.java.dev.jna:jna:4.4.0"), None);
        assert_eq!(jna_needs_bump("net.java.dev.jna:jna:3.4.0"), None);
        assert_eq!(jna_needs_bump("net.java.dev.jna:platform:3.4.0"), None);
        assert_eq!(jna_needs_bump("org.lwjgl:lwjgl:3.3.3"), None);
    }

    #[test]
    fn test_jna_compat_rel_path() {
        assert_eq!(
            jna_compat_rel_path("jna"),
            PathBuf::from("net/java/dev/jna/jna/5.13.0/jna-5.13.0.jar")
        );
    }

    #[test]
    fn test_native_classifier_key_from_natives_map() {
        use crate::mc_types::McLibrary;
        let os_key = match OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let lib = McLibrary {
            name: "org.lwjgl.lwjgl:lwjgl-platform:2.9.4".into(),
            downloads: None,
            rules: None,
            natives: Some(HashMap::from([(
                os_key.to_string(),
                format!("natives-{os_key}-${{arch}}"),
            )])),
            extract: None,
        };
        let key = native_classifier_key(&lib).expect("應有 natives key");
        assert!(
            key == format!("natives-{os_key}-64") || key == format!("natives-{os_key}-32"),
            "arch 未替換：{key}"
        );
    }

    #[test]
    fn test_native_classifier_key_fallback_to_classifiers() {
        // 1.12.2 測試資料經過正規化，natives 欄位遺失 → 走 classifiers 猜測
        let v = load_version("data/1.12.2.json");
        let with_natives: Vec<&str> = v
            .libraries
            .iter()
            .filter(|l| native_classifier_key(l).is_some())
            .map(|l| l.name.as_str())
            .collect();
        assert!(
            with_natives
                .iter()
                .any(|n| n.contains("lwjgl-platform") || n.contains("jinput-platform")),
            "1.12.2 應偵測到 natives classifier，實際：{with_natives:?}"
        );
    }

    #[test]
    fn test_new_format_has_no_classifier_natives() {
        // 1.21 的 natives 是獨立 artifact library，不應誤判為 classifier natives
        let v = load_version("data/1.21.json");
        assert!(
            v.libraries.iter().all(|l| native_classifier_key(l).is_none()),
            "新版格式不應有 classifier natives"
        );
    }
}
