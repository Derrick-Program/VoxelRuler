use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use std::env::consts::{ARCH, OS};

use crate::mc_types::{
    McArgumentItem, McArgumentValue, McFeatureRule, McOsRule, McRule, McRuleAction, McRuleArch,
    McRuleOS, McSpecificVersionDetail,
};

// ── LaunchContext ─────────────────────────────────────────────────────────────

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

        cmd.arg(format!("-Xmx{}", self.xmx));
        cmd.arg(format!("-Xms{}", self.xms));

        if let Some(arguments) = &self.version.arguments {
            if let Some(default_jvm) = &arguments.default_user_jvm {
                for item in default_jvm {
                    apply_arg(&mut cmd, item, &vars);
                }
            }
            for item in &arguments.jvm {
                apply_arg(&mut cmd, item, &vars);
            }
        } else {
            // Pre-1.13 versions have no structured jvm arguments
            cmd.arg(format!("-Djava.library.path={}", self.natives_dir.display()));
            cmd.arg("-cp");
            cmd.arg(&classpath);
        }

        cmd.arg(&self.version.main_class);

        if let Some(arguments) = &self.version.arguments {
            for item in &arguments.game {
                apply_arg(&mut cmd, item, &vars);
            }
        } else if let Some(mc_args) = &self.version.minecraft_arguments {
            for part in mc_args.split_whitespace() {
                cmd.arg(resolve_argument(part, &vars));
            }
        }

        cmd
    }

    fn build_classpath(&self) -> String {
        let sep = if cfg!(windows) { ";" } else { ":" };
        let mut parts: Vec<String> = Vec::new();

        for lib in &self.version.libraries {
            if let Some(rules) = &lib.rules {
                if !evaluate_rules(rules) {
                    continue;
                }
            }

            let path = lib
                .downloads
                .as_ref()
                .and_then(|d| d.artifact.as_ref())
                .and_then(|a| a.path.as_ref())
                .map(|p| self.libraries_dir.join(p).to_string_lossy().into_owned())
                .or_else(|| {
                    maven_coord_to_path(&lib.name)
                        .map(|p| self.libraries_dir.join(p).to_string_lossy().into_owned())
                });

            if let Some(p) = path {
                parts.push(p);
            }
        }

        parts.push(
            self.versions_dir
                .join(&self.version.id)
                .join(format!("{}.jar", self.version.id))
                .to_string_lossy()
                .into_owned(),
        );

        parts.join(sep)
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
        m.insert("assets_root", self.assets_dir.to_string_lossy().into_owned());
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

// ── Rule evaluation ───────────────────────────────────────────────────────────

fn evaluate_rules(rules: &[McRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().map_or(true, os_rule_matches);
        let feat_ok = rule.features.as_ref().map_or(true, feature_rule_matches);
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

// ── Argument helpers ──────────────────────────────────────────────────────────

fn apply_arg(cmd: &mut Command, item: &McArgumentItem, vars: &HashMap<&'static str, String>) {
    match item {
        McArgumentItem::Simple(s) => {
            cmd.arg(resolve_argument(s, vars));
        }
        McArgumentItem::Conditional(cond) => {
            if evaluate_rules(&cond.rules) {
                match &cond.value {
                    McArgumentValue::Single(s) => {
                        cmd.arg(resolve_argument(s, vars));
                    }
                    McArgumentValue::Many(args) => {
                        for a in args {
                            cmd.arg(resolve_argument(a, vars));
                        }
                    }
                }
            }
        }
    }
}

// ── Maven coordinate → relative path ─────────────────────────────────────────

/// Converts a Maven coordinate (`group:artifact:version[:classifier]`) to a
/// relative path suitable for joining with `libraries_dir`.
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

// ─────────────────────────────────────────────────────────────────────────────

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
            eprintln!("警告：未知的系統或架構组合 OS: {}, ARCH: {}", OS, ARCH);
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
        let data = std::fs::read_to_string("data/26.1.2.json").expect("can't read file");
        match serde_json::from_str::<McSpecificVersionDetail>(&data) {
            Ok(v) => {
                println!("解析成功！");
                println!("完整結構體: {:#?}", v.arguments.unwrap().jvm);
            }
            Err(e) => {
                println!("解析失敗: {}", e);
            }
        }
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
      let java_version = java_all.get(os_arch).unwrap().get("java-runtime-delta").unwrap();
      let java_manifest = &java_version.first().unwrap().manifest;
      println!("Minecraft Java Manifest: {:#?}", java_manifest);
      let url = java_manifest.url.clone();
      println!("Minecraft Java 下載 URL: {}", url);
      let response: crate::mc_types::McJavaManifest = reqwest::get(&url).await.expect("下載失敗").error_for_status().expect("HTTP 錯誤").json().await.expect("解析 JSON 失敗");
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

    // 1.21 (modern)：從真實 JSON 解析，驗證 structured arguments 與 rule 評估
    #[test]
    fn test_build_command_from_1_21_json() {
        let data = std::fs::read_to_string("data/1.21.json").expect("找不到 data/1.21.json");
        let version: McSpecificVersionDetail =
            serde_json::from_str(&data).expect("解析 1.21.json 失敗");
        let cmd = make_ctx(version).build_command();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

        // Heap flags
        assert!(args.contains(&"-Xmx2G".into()), "缺少 -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "缺少 -Xms");

        // Unconditional JVM args：變數替換正確
        assert!(args.contains(&"-Djava.library.path=/natives".into()), "natives_directory 未替換");
        assert!(args.contains(&"-Djna.tmpdir=/natives".into()), "natives_directory 未替換（jna）");
        assert!(args.contains(&"-cp".into()), "缺少 -cp");

        // Classpath：版本 JAR 在最後
        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        let classpath = &args[cp_pos + 1];
        assert!(classpath.ends_with("1.21/1.21.jar"), "classpath 應以版本 JAR 結尾");

        // OS-conditional 函式庫：macOS 包含 java-objc-bridge，排除 linux/windows natives
        #[cfg(target_os = "macos")]
        {
            assert!(classpath.contains("java-objc-bridge"), "macOS classpath 應包含 java-objc-bridge");
            assert!(!classpath.contains("natives-linux"), "macOS classpath 不應有 linux natives");
            assert!(!classpath.contains("natives-windows"), "macOS classpath 不應有 windows natives");
        }
        #[cfg(target_os = "linux")]
        {
            assert!(classpath.contains("natives-linux"), "Linux classpath 應包含 linux natives");
            assert!(!classpath.contains("natives-macos"), "Linux classpath 不應有 macos natives");
        }
        #[cfg(target_os = "windows")]
        {
            assert!(classpath.contains("natives-windows"), "Windows classpath 應包含 windows natives");
            assert!(!classpath.contains("natives-linux"), "Windows classpath 不應有 linux natives");
        }

        // Main class
        assert!(args.contains(&"net.minecraft.client.main.Main".into()), "缺少 main class");

        // Game args：無條件項目都在，變數替換正確
        assert!(args.contains(&"--username".into()));
        assert!(args.contains(&"Steve".into()), "auth_player_name 未替換");
        assert!(args.contains(&"1.21".into()), "version_name 未替換");
        assert!(args.contains(&"--gameDir".into()));
        assert!(args.contains(&"/game".into()), "game_directory 未替換");
        assert!(args.contains(&"msa".into()), "user_type 應為 msa");

        // Feature-gated game args：不應出現（feature_rule_matches 全部回傳 false）
        assert!(!args.contains(&"--demo".into()), "--demo 不應出現（非 demo 模式）");
        assert!(!args.contains(&"--width".into()), "--width 不應出現（無自訂解析度）");
        assert!(!args.contains(&"--quickPlayPath".into()), "--quickPlayPath 不應出現");

        // OS-conditional JVM args
        #[cfg(target_os = "macos")]
        assert!(args.contains(&"-XstartOnFirstThread".into()), "macOS 應有 -XstartOnFirstThread");
        #[cfg(not(target_os = "macos"))]
        assert!(!args.contains(&"-XstartOnFirstThread".into()), "非 macOS 不應有 -XstartOnFirstThread");
        #[cfg(not(target_os = "windows"))]
        assert!(!args.iter().any(|a| a.contains("HeapDumpPath")), "非 Windows 不應有 HeapDumpPath");
        dbg!(&args);
        dbg!(&cmd);
    }

    // 1.12.2 (legacy)：從真實 JSON 解析，驗證 minecraft_arguments 展開與 rule 評估
    #[test]
    fn test_build_command_from_1_12_2_json() {
        let data = std::fs::read_to_string("data/1.12.2.json").expect("找不到 data/1.12.2.json");
        let version: McSpecificVersionDetail =
            serde_json::from_str(&data).expect("解析 1.12.2.json 失敗");
        let cmd = make_ctx(version).build_command();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

        // Heap flags
        assert!(args.contains(&"-Xmx2G".into()), "缺少 -Xmx");
        assert!(args.contains(&"-Xms512M".into()), "缺少 -Xms");

        // Legacy JVM 路徑
        assert!(args.contains(&"-Djava.library.path=/natives".into()), "缺少 natives_directory");
        assert!(args.contains(&"-cp".into()), "缺少 -cp");

        // Classpath：版本 JAR 在最後
        let cp_pos = args.iter().position(|a| a == "-cp").expect("找不到 -cp");
        let classpath = &args[cp_pos + 1];
        assert!(classpath.ends_with("1.12.2/1.12.2.jar"), "classpath 應以版本 JAR 結尾");

        // 1.12.2 對 lwjgl 有兩套：2.9.4（[allow all, disallow osx]）和 2.9.2（[allow osx]）
        // macOS：2.9.4 被 disallow，2.9.2 被 allow
        #[cfg(target_os = "macos")]
        {
            assert!(!classpath.contains("lwjgl-2.9.4"), "macOS: lwjgl 2.9.4 應被 disallow 排除");
            assert!(classpath.contains("lwjgl-2.9.2"), "macOS: lwjgl 2.9.2 應被 allow 包含");
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(classpath.contains("lwjgl-2.9.4"), "非 macOS: lwjgl 2.9.4 應被包含");
            assert!(!classpath.contains("lwjgl-2.9.2"), "非 macOS: lwjgl 2.9.2（macOS 專用）應被排除");
        }

        // Main class
        assert!(args.contains(&"net.minecraft.client.main.Main".into()), "缺少 main class");

        // minecraft_arguments 展開：所有變數替換正確
        assert!(args.contains(&"--username".into()));
        assert!(args.contains(&"Steve".into()), "auth_player_name 未替換");
        assert!(args.contains(&"1.12.2".into()), "version_name 未替換");
        assert!(args.contains(&"--gameDir".into()));
        assert!(args.contains(&"/game".into()), "game_directory 未替換");
        assert!(args.contains(&"--userType".into()));
        assert!(args.contains(&"msa".into()), "user_type 應為 msa");
        assert!(args.contains(&"--uuid".into()));
        assert!(args.contains(&"uuid-1234".into()), "auth_uuid 未替換");
        dbg!(&args);
        dbg!(&cmd);
    }
}
