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
/// 回傳 artifact 名稱（`"jna"` or `"jna-platform"`）。
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
    /// 實際使用的 Java major 版本（自訂 Java 時偵測而得）。
    /// `None` 時 fallback 至版本 JSON 的 javaVersion.majorVersion。
    pub java_major_version: Option<i32>,
    /// macOS Apple Silicon 原生模式的函式庫替換表（Prism 式）
    pub compat_override: Option<&'static crate::mc_compat::MacosArm64Override>,
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
            all_jvm.push(format!(
                "-Dminecraft.applet.TargetDirectory={}",
                self.game_dir.display()
            ));
            all_jvm.push("-Dfml.ignorePatchDiscrepancies=true".to_string());
            all_jvm.push("-Dfml.ignoreInvalidMinecraftCertificates=true".to_string());

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
                "-Dminecraft.applet.TargetDirectory={}",
                self.game_dir.display()
            ));
            cmd.arg("-Dfml.ignorePatchDiscrepancies=true");
            cmd.arg("-Dfml.ignoreInvalidMinecraftCertificates=true");
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

            // Apple Silicon 原生模式：被替換的 lib 不上 classpath
            if let Some(ov) = self.compat_override
                && ov.excludes(&lib.name)
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
                // 但若有 natives 欄位，這是舊版格式的 natives-only 函式庫，不上 classpath
                None => {
                    if lib.natives.is_some() {
                        None
                    } else {
                        maven_coord_to_path(&lib.name)
                    }
                }
            };
            if let Some(r) = rel {
                parts.push(self.libraries_dir.join(r));
            }
        }

        // Apple Silicon 原生模式：替換用的 jar 上 classpath
        if let Some(ov) = self.compat_override {
            for art in ov.artifacts.iter().filter(|a| !a.extract) {
                parts.push(self.libraries_dir.join(art.rel_path));
            }
        }

        let version_jar = self
            .versions_dir
            .join(&self.version.id)
            .join(format!("{}.jar", self.version.id));
        let nosig_jar = self
            .versions_dir
            .join(&self.version.id)
            .join(format!("{}-nosig.jar", self.version.id));
        parts.push(if nosig_jar.exists() {
            nosig_jar
        } else {
            version_jar
        });

        // 同名 lib 可能因規則重複通過（如 1.18.x 的 lwjgl）→ 去重保留首見順序
        let mut seen = std::collections::HashSet::new();
        parts.retain(|p| seen.insert(p.clone()));

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
        // compat flags 必須依「實際執行的 Java」決定：
        // 例如 1.18.1（建議 Java 17）配自訂 Java 8 時，
        // 不能塞 --add-modules=jdk.incubator.vector（Java 8 不認識 → JVM 起不來）
        let major = self.java_major_version.unwrap_or_else(|| {
            self.version
                .java_version
                .as_ref()
                .map_or(8, |jv| jv.major_version)
        });

        let mut args: Vec<String> = Vec::new();
        if major >= 17 {
            args.push("--add-modules=jdk.incubator.vector".into());
            args.push("--enable-native-access=ALL-UNNAMED".into());
        }
        if major >= 22 {
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
        // 1.7.x–1.8.x 的 --userProperties：必須是合法 JSON（空物件），
        // 給空字串會讓舊版 Main.main 的 gson 解析回傳 null → NPE
        m.insert("user_properties", "{}".into());
        m.insert("user_property_map", "{}".into());
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
        if let Some((pi, _)) = UNIQUE_PREFIXES
            .iter()
            .enumerate()
            .find(|(_, p)| arg.starts_with(*p))
        {
            last_prefix[pi] = Some(i);
        }
    }

    // Pass 2：過濾，重複的非最後那筆記錄 warning 並移除
    args.into_iter()
        .enumerate()
        .filter_map(|(i, arg)| {
            if GC_FLAGS.contains(&arg.as_str()) {
                if Some(i) != last_gc {
                    warn!("Removed conflicting GC argument: {arg}");
                    return None;
                }
                return Some(arg);
            }
            if let Some((pi, _)) = UNIQUE_PREFIXES
                .iter()
                .enumerate()
                .find(|(_, p)| arg.starts_with(*p))
            {
                if Some(i) != last_prefix[pi] {
                    warn!("Removing duplicate JVM argument: {arg}");
                    return None;
                }
                return Some(arg);
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
    items
        .iter()
        .filter_map(|item| match item {
            McArgumentItem::Simple(s) => Some(vec![resolve_argument(s, vars)]),
            McArgumentItem::Conditional(cond) if evaluate_rules(&cond.rules) => {
                Some(match &cond.value {
                    McArgumentValue::Single(s) => vec![resolve_argument(s, vars)],
                    McArgumentValue::Many(args) => {
                        args.iter().map(|a| resolve_argument(a, vars)).collect()
                    }
                })
            }
            _ => None,
        })
        .flatten()
        .collect()
}

pub fn maven_coord_to_path(coord: &str) -> Option<PathBuf> {
    let mut parts = coord.splitn(4, ':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    let mut path: PathBuf = group.split('.').collect();
    path.push(artifact);
    path.push(version);
    let filename = match classifier {
        Some(cls) => format!("{artifact}-{version}-{cls}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    path.push(filename);
    Some(path)
}

/// 此版本是否提供 macOS arm64（Apple Silicon）原生函式庫。
/// 1.19+ 的版本 JSON 會包含 `natives-macos-arm64` 的 lwjgl 條目；
/// 1.18.x 以前只有 x86_64，必須用 x64 Java 透過 Rosetta 執行。
pub fn version_supports_macos_arm64(version: &McSpecificVersionDetail) -> bool {
    version.libraries.iter().any(|lib| {
        lib.name.contains("natives-macos-arm64")
            || lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .is_some_and(|c| c.contains_key("natives-macos-arm64"))
    })
}

/// 偵測 Mach-O 執行檔支援的架構（macOS；透過 `lipo -archs`）
#[cfg(target_os = "macos")]
pub fn detect_java_archs(java_path: &std::path::Path) -> Vec<String> {
    Command::new("/usr/bin/lipo")
        .arg("-archs")
        .arg(java_path)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(|s| s.to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// 執行 `java -version` 偵測實際 Java major 版本（如 8 / 17 / 21）
pub fn detect_java_major_version(java_path: &std::path::Path) -> Option<i32> {
    let output = Command::new(java_path).arg("-version").output().ok()?;
    // `java -version` 輸出在 stderr；保險起見 stdout 也試
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let text = if stderr.contains("version") {
        stderr
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    parse_java_major_version(&text)
}

/// 解析 `version "..."` string: `"1.8.0_392"` → 8、`"17.0.2"` → 17、`"21"` → 21
fn parse_java_major_version(text: &str) -> Option<i32> {
    let start = text.find("version \"")? + "version \"".len();
    let quoted = text[start..].split('"').next()?;
    let mut nums = quoted.split(['.', '_', '-', '+']);
    let first: i32 = nums.next()?.trim().parse().ok()?;
    if first == 1 {
        // 舊式 1.x 命名（Java 8 以前）
        nums.next()?.trim().parse().ok()
    } else {
        Some(first)
    }
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
            warn!(
                os = OS,
                arch = ARCH,
                "Unknown OS or architecture combination"
            );
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
        let data =
            std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Could not find {path}"));
        serde_json::from_str(&data).unwrap_or_else(|_| panic!("Failed to parse {path}"))
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
        println!("Complete struct: {:#?}", v.arguments.unwrap().jvm);
    }

    #[tokio::test]
    async fn test_parse_java_all() {
        let data = tokio::fs::read_to_string("data/java-all.json")
            .await
            .expect("can't read file");
        let java_all: McJavaAll = serde_json::from_str(&data).expect("Parse failed");
        println!("Parse successful!");
        println!("Complete struct: {:#?}", java_all);
        let os_arch = get_mojang_os_arch();
        println!("Mojang string for current OS arch: {}", os_arch);
        let mac_java = java_all.get(os_arch).unwrap();
        println!("{} Java version: {:#?}", os_arch, mac_java);
    }

    #[tokio::test]
    async fn test_java_parse() {
        let os_arch = get_mojang_os_arch();
        println!("Mojang string for current OS arch: {}", os_arch);
        let data = tokio::fs::read_to_string("data/java-all.json")
            .await
            .expect("can't read file");
        let java_all: McJavaAll = serde_json::from_str(&data).expect("Parse failed");
        let java_version = java_all
            .get(os_arch)
            .unwrap()
            .get("java-runtime-delta")
            .unwrap();
        let java_manifest = &java_version.first().unwrap().manifest;
        println!("Minecraft Java Manifest: {:#?}", java_manifest);
        let url = java_manifest.url.clone();
        println!("Minecraft Java download URL: {}", url);
        let response: crate::mc_types::McJavaManifest = reqwest::get(&url)
            .await
            .expect("Download failed")
            .error_for_status()
            .expect("HTTP error")
            .json()
            .await
            .expect("Failed to parse JSON");
        println!("Minecraft Java Manifest content: {:#?}", response);
    }

    #[test]
    fn test_get_mojang_os_arch() {
        let os_arch = get_mojang_os_arch();
        println!("Mojang string for current OS arch: {}", os_arch);
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
            java_major_version: None,
            compat_override: None,
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
    fn test_java22_compat_args_injected_before_cp() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 22).build_command();
        let args = cmd_args(&cmd);

        let cp_pos = args
            .iter()
            .position(|a| a == "-cp")
            .expect("Could not find -cp");
        assert!(
            args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Missing incubator.vector"
        );
        assert!(
            args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Missing native-access"
        );
        assert!(
            args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "Missing unsafe-memory-access"
        );

        let native_pos = args
            .iter()
            .position(|a| a == "--enable-native-access=ALL-UNNAMED")
            .unwrap();
        let unsafe_pos = args
            .iter()
            .position(|a| a == "--sun-misc-unsafe-memory-access=allow")
            .unwrap();
        assert!(
            native_pos < cp_pos,
            "--enable-native-access should be before -cp"
        );
        assert!(
            unsafe_pos < cp_pos,
            "--sun-misc-unsafe-memory-access should be before -cp"
        );
    }

    #[test]
    fn test_java17_compat_args_no_unsafe_memory_access() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 17).build_command();
        let args = cmd_args(&cmd);

        let cp_pos = args
            .iter()
            .position(|a| a == "-cp")
            .expect("Could not find -cp");
        assert!(
            args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Java 17 should have incubator.vector"
        );
        assert!(
            args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Java 17 should have native-access"
        );
        assert!(
            !args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "Java 17 should not have unsafe-memory-access (requires >= 22)"
        );

        let native_pos = args
            .iter()
            .position(|a| a == "--enable-native-access=ALL-UNNAMED")
            .unwrap();
        assert!(
            native_pos < cp_pos,
            "--enable-native-access should be before -cp"
        );
    }

    #[test]
    fn test_old_java_no_compat_args() {
        let cmd = make_ctx_with_java(load_version("data/1.21.json"), 8).build_command();
        let args = cmd_args(&cmd);

        assert!(
            !args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Java 8 should not have compat args"
        );
        assert!(
            !args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Java 8 should not have compat args"
        );
        assert!(
            !args.contains(&"--sun-misc-unsafe-memory-access=allow".into()),
            "Java 8 should not have compat args"
        );
    }

    #[test]
    fn test_build_command_from_1_21_json() {
        let cmd = make_ctx(load_version("data/1.21.json")).build_command();
        let args = cmd_args(&cmd);

        assert!(args.contains(&"-Xmx2G".into()), "Missing -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "Missing -Xms");
        assert!(
            args.contains(&"-Djava.library.path=/natives".into()),
            "natives_directory not replaced"
        );
        assert!(
            args.contains(&"-Djna.tmpdir=/natives".into()),
            "natives_directory not replaced (jna)"
        );
        assert!(args.contains(&"-cp".into()), "Missing -cp");

        let cp_pos = args
            .iter()
            .position(|a| a == "-cp")
            .expect("Could not find -cp");
        let classpath = &args[cp_pos + 1];
        assert!(
            classpath.ends_with("1.21/1.21.jar"),
            "classpath should end with version JAR"
        );

        #[cfg(target_os = "macos")]
        {
            assert!(
                classpath.contains("java-objc-bridge"),
                "macOS classpath should contain java-objc-bridge"
            );
            assert!(
                !classpath.contains("natives-linux"),
                "macOS classpath should not contain linux natives"
            );
            assert!(
                !classpath.contains("natives-windows"),
                "macOS classpath should not contain windows natives"
            );
        }
        #[cfg(target_os = "linux")]
        {
            assert!(
                classpath.contains("natives-linux"),
                "Linux classpath should contain linux natives"
            );
            assert!(
                !classpath.contains("natives-macos"),
                "Linux classpath should not contain macos natives"
            );
        }
        #[cfg(target_os = "windows")]
        {
            assert!(
                classpath.contains("natives-windows"),
                "Windows classpath should contain windows natives"
            );
            assert!(
                !classpath.contains("natives-linux"),
                "Windows classpath should not contain linux natives"
            );
        }

        assert!(
            args.contains(&"net.minecraft.client.main.Main".into()),
            "Missing main class"
        );

        assert!(args.contains(&"--username".into()));
        assert!(
            args.contains(&"Steve".into()),
            "auth_player_name not replaced"
        );
        assert!(args.contains(&"1.21".into()), "version_name not replaced");
        assert!(args.contains(&"--gameDir".into()));
        assert!(
            args.contains(&"/game".into()),
            "game_directory not replaced"
        );
        assert!(args.contains(&"msa".into()), "user_type should be msa");

        assert!(
            !args.contains(&"--demo".into()),
            "--demo should not appear (not demo mode)"
        );
        assert!(
            !args.contains(&"--width".into()),
            "--width should not appear (no custom resolution)"
        );
        assert!(
            !args.contains(&"--quickPlayPath".into()),
            "--quickPlayPath should not appear"
        );

        #[cfg(target_os = "macos")]
        assert!(
            args.contains(&"-XstartOnFirstThread".into()),
            "macOS should have -XstartOnFirstThread"
        );
        #[cfg(not(target_os = "macos"))]
        assert!(
            !args.contains(&"-XstartOnFirstThread".into()),
            "Non-macOS should not have -XstartOnFirstThread"
        );
        #[cfg(not(target_os = "windows"))]
        assert!(
            !args.iter().any(|a| a.contains("HeapDumpPath")),
            "Non-Windows should not have HeapDumpPath"
        );
    }

    #[test]
    fn test_build_command_from_1_12_2_json() {
        let cmd = make_ctx(load_version("data/1.12.2.json")).build_command();
        let args = cmd_args(&cmd);

        assert!(args.contains(&"-Xmx2G".into()), "Missing -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "Missing -Xms");

        assert!(
            args.contains(&"-Djava.library.path=/natives".into()),
            "Missing natives_directory"
        );
        assert!(args.contains(&"-cp".into()), "Missing -cp");

        let cp_pos = args
            .iter()
            .position(|a| a == "-cp")
            .expect("Could not find -cp");
        let classpath = &args[cp_pos + 1];
        assert!(
            classpath.ends_with("1.12.2/1.12.2.jar"),
            "classpath should end with version JAR"
        );

        #[cfg(target_os = "macos")]
        {
            assert!(
                !classpath.contains("lwjgl-2.9.4"),
                "macOS: lwjgl 2.9.4 should be excluded by disallow"
            );
            assert!(
                classpath.contains("lwjgl-2.9.2"),
                "macOS: lwjgl 2.9.2 should be included by allow"
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(
                classpath.contains("lwjgl-2.9.4"),
                "Non-macOS: lwjgl 2.9.4 should be included"
            );
            assert!(
                !classpath.contains("lwjgl-2.9.2"),
                "Non-macOS: lwjgl 2.9.2 (macOS only) should be excluded"
            );
        }

        assert!(
            args.contains(&"net.minecraft.client.main.Main".into()),
            "Missing main class"
        );

        assert!(args.contains(&"--username".into()));
        assert!(
            args.contains(&"Steve".into()),
            "auth_player_name not replaced"
        );
        assert!(args.contains(&"1.12.2".into()), "version_name not replaced");
        assert!(args.contains(&"--gameDir".into()));
        assert!(
            args.contains(&"/game".into()),
            "game_directory not replaced"
        );
        assert!(args.contains(&"--userType".into()));
        assert!(args.contains(&"msa".into()), "user_type should be msa");
        assert!(args.contains(&"--uuid".into()));
        assert!(args.contains(&"uuid-1234".into()), "auth_uuid not replaced");
    }

    #[test]
    fn test_parse_java_major_version() {
        assert_eq!(
            parse_java_major_version(r#"openjdk version "1.8.0_392""#),
            Some(8)
        );
        assert_eq!(
            parse_java_major_version(r#"openjdk version "17.0.2" 2022-01-18"#),
            Some(17)
        );
        assert_eq!(
            parse_java_major_version(r#"java version "21" 2023-09-19 LTS"#),
            Some(21)
        );
        assert_eq!(
            parse_java_major_version(r#"openjdk version "21.0.1+12-LTS""#),
            Some(21)
        );
        assert_eq!(parse_java_major_version("no version here"), None);
    }

    #[test]
    fn test_compat_args_follow_actual_java_not_version_requirement() {
        // 1.21 建議 Java 21，但實際用 Java 8 → 不得出現 compat flags
        let mut ctx = make_ctx(load_version("data/1.21.json"));
        ctx.java_major_version = Some(8);
        let args = cmd_args(&ctx.build_command());
        assert!(
            !args.contains(&"--add-modules=jdk.incubator.vector".into()),
            "Actual Java 8 should not have incubator.vector"
        );
        assert!(
            !args.contains(&"--enable-native-access=ALL-UNNAMED".into()),
            "Actual Java 8 should not have native-access"
        );

        // 實際 Java 22 → 應有完整 compat flags (含 sun-misc)
        ctx.java_major_version = Some(22);
        let args = cmd_args(&ctx.build_command());
        assert!(args.contains(&"--add-modules=jdk.incubator.vector".into()));
        assert!(args.contains(&"--sun-misc-unsafe-memory-access=allow".into()));
    }

    #[test]
    fn test_1_7_10_user_properties_is_valid_json() {
        let cmd = make_ctx(load_version("data/1.7.10.json")).build_command();
        let args = cmd_args(&cmd);
        let pos = args
            .iter()
            .position(|a| a == "--userProperties")
            .expect("1.7.10 should have --userProperties");
        assert_eq!(
            args[pos + 1],
            "{}",
            "--userProperties must be empty JSON object, empty string causes NPE in old Main"
        );
    }

    #[test]
    fn test_compat_override_replaces_lwjgl_in_classpath() {
        let mut ctx = make_ctx(load_version("data/1.18.1.json"));
        ctx.compat_override = crate::mc_compat::arm64_override_for(&ctx.version);
        assert!(
            ctx.compat_override.is_some(),
            "1.18.1 should have override table"
        );

        let cp = ctx
            .classpath_paths()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(cp.contains("lwjgl-3.3.1.jar"), "Should have lwjgl 3.3.1");
        assert!(
            cp.contains("lwjgl-glfw-3.3.1-natives-macos-arm64.jar"),
            "Should have arm64 natives"
        );
        assert!(
            !cp.contains("3.2.1"),
            "Original lwjgl 3.2.1 should be excluded"
        );
        assert!(
            cp.contains("java-objc-bridge-1.1"),
            "Should be replaced with java-objc-bridge 1.1"
        );
        assert!(
            !cp.contains("java-objc-bridge-1.0.0"),
            "java-objc-bridge 1.0.0 should be excluded"
        );
    }

    #[test]
    fn test_version_supports_macos_arm64() {
        assert!(
            version_supports_macos_arm64(&load_version("data/1.21.json")),
            "1.21 should support Apple Silicon"
        );
        assert!(
            version_supports_macos_arm64(&load_version("data/1.19.2.json")),
            "1.19.2 should support Apple Silicon"
        );
        assert!(
            !version_supports_macos_arm64(&load_version("data/1.18.1.json")),
            "1.18.1 does not support Apple Silicon (x86_64 natives only)"
        );
        assert!(
            !version_supports_macos_arm64(&load_version("data/1.12.2.json")),
            "1.12.2 does not support Apple Silicon"
        );
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
        let key = native_classifier_key(&lib).expect("Should have natives key");
        assert!(
            key == format!("natives-{os_key}-64") || key == format!("natives-{os_key}-32"),
            "arch not replaced: {key}"
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
            "1.12.2 should detect natives classifier, actual: {with_natives:?}"
        );
    }

    #[test]
    fn test_new_format_has_no_classifier_natives() {
        // 1.21 的 natives 是獨立 artifact library，不應誤判為 classifier natives
        let v = load_version("data/1.21.json");
        assert!(
            v.libraries
                .iter()
                .all(|l| native_classifier_key(l).is_none()),
            "New format should not have classifier natives"
        );
    }
}
