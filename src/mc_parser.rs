use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use std::env::consts::{ARCH, OS};

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

pub fn resolve_argument(arg: &str, vars: &HashMap<&str, String>) -> String {
    let re = get_macro_regex();
    re.replace_all(arg, |caps: &regex::Captures| {
        let key = &caps[1];
        vars.get(key).cloned().unwrap_or_else(|| "".to_string())
    })
    .into_owned()
}

mod test {
    use super::*;
    use crate::mc_types::{McJavaAll, McSpecificVersionDetail};
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
}
