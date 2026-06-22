#![allow(unused)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct McVersionInfo {
    pub latest: McLatestVersion,
    pub versions: Vec<McVersion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLatestVersion {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McVersion {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: String,
    #[serde(rename = "complianceLevel")]
    pub compliance_level: i32,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct McProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<McProfileSkin>,
    pub capes: Vec<McProfileCape>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum McState {
    Active,
    Inactive,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum McSkinVariant {
    Classic,
    Slim,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McProfileSkin {
    pub id: String,
    pub state: McState,
    #[serde(rename = "textureKey")]
    pub texture_key: String,
    pub url: String,
    pub variant: McSkinVariant,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McProfileCape {
    pub id: String,
    pub state: McState,
    pub url: String,
    pub alias: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McSpecificVersionDetail {
    pub id: String,
    pub r#type: String,
    pub time: String,
    pub release_time: String,
    pub compliance_level: Option<i32>,
    pub minimum_launcher_version: Option<i32>,
    pub main_class: String,
    pub java_version: Option<McJavaVersion>,
    pub downloads: Option<McVersionDownloads>,
    pub asset_index: Option<McAssetIndex>,
    pub assets: Option<String>,
    pub logging: Option<HashMap<String, McLoggingConfig>>,
    pub libraries: Vec<McLibrary>,
    pub arguments: Option<McArguments>,
    pub minecraft_arguments: Option<String>,
}

impl McSpecificVersionDetail {
    pub fn merge(mut self, modded: Self) -> Self {
        self.id = modded.id;
        self.r#type = modded.r#type;
        self.time = modded.time;
        self.release_time = modded.release_time;
        self.main_class = modded.main_class;
        
        if let Some(jv) = modded.java_version { self.java_version = Some(jv); }
        if let Some(dl) = modded.downloads { self.downloads = Some(dl); }
        if let Some(ai) = modded.asset_index { self.asset_index = Some(ai); }
        if let Some(ass) = modded.assets { self.assets = Some(ass); }
        if let Some(log) = modded.logging { self.logging = Some(log); }
        
        // 優先讀取 ModLoader 的 libraries，原版墊後
        let mut new_libs = modded.libraries;
        new_libs.extend(self.libraries);
        self.libraries = new_libs;

        // 合併 arguments
        match (self.arguments.as_mut(), modded.arguments) {
            (Some(vanilla_args), Some(modded_args)) => {
                vanilla_args.game.extend(modded_args.game);
                vanilla_args.jvm.extend(modded_args.jvm);
                // 這裡我們簡單把 modded args 放在後面
                // 通常 modloader 會把必要的參數放到 jvm args，所以直接 append 是沒問題的
            }
            (None, Some(modded_args)) => {
                self.arguments = Some(modded_args);
            }
            _ => {}
        }
        
        if let Some(mc_args) = modded.minecraft_arguments {
            self.minecraft_arguments = Some(mc_args);
        }

        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McJavaVersion {
    pub component: String,
    pub major_version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McVersionDownloads {
    pub client: Option<McArtifactInfo>,
    pub server: Option<McArtifactInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McAssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLoggingConfig {
    pub argument: String,
    pub file: McLogFile,
    pub r#type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLogFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLibrary {
    pub name: String,
    pub downloads: Option<McLibraryDownloads>,
    pub rules: Option<Vec<McRule>>,
    /// 舊版格式（約 ≤1.18）：OS 名稱 → classifier key（可能含 `${arch}`），
    /// 例如 `{"osx": "natives-osx", "windows": "natives-windows-${arch}"}`
    pub natives: Option<HashMap<String, String>>,
    /// 舊版格式：natives jar 解壓規則
    pub extract: Option<McExtract>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McExtract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLibraryDownloads {
    pub artifact: Option<McArtifactInfo>,
    pub classifiers: Option<HashMap<String, McArtifactInfo>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McArtifactInfo {
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum McRuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McRule {
    pub action: McRuleAction,
    pub os: Option<McOsRule>,
    pub features: Option<McFeatureRule>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum McRuleOS {
    Windows,
    Osx,
    Linux,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum McRuleArch {
    X86,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McOsRule {
    pub name: Option<McRuleOS>,
    pub arch: Option<McRuleArch>,
    pub version: Option<String>,
    #[serde(rename = "versionRange")]
    pub version_range: Option<McVersionRange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McVersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McFeatureRule {
    pub is_demo_user: Option<bool>,
    pub has_custom_resolution: Option<bool>,
    pub has_quick_plays_support: Option<bool>,
    pub is_quick_play_singleplayer: Option<bool>,
    pub is_quick_play_multiplayer: Option<bool>,
    pub is_quick_play_realms: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct McArguments {
    #[serde(rename = "default-user-jvm")]
    pub default_user_jvm: Option<Vec<McArgumentItem>>,
    pub game: Vec<McArgumentItem>,
    pub jvm: Vec<McArgumentItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McArgumentItem {
    Simple(String),
    Conditional(McConditionalArgument),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McConditionalArgument {
    #[serde(default)]
    pub rules: Vec<McRule>,
    pub value: McArgumentValue,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McArgumentValue {
    Single(String),
    Many(Vec<String>),
}

pub type McJavaAll = HashMap<String, HashMap<String, Vec<McJavaRuntime>>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaRuntime {
    pub availability: McJavaAvailability,
    pub manifest: McJavaManifestInfo,
    pub version: McJavaRuntimeVersion,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaAvailability {
    pub group: i32,
    pub progress: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaManifestInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaRuntimeVersion {
    pub name: String,
    pub released: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaManifest {
    pub files: HashMap<String, McJavaFileEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McJavaFileEntry {
    File {
        #[serde(default)]
        executable: bool,
        downloads: McJavaFileDownloads,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaFileDownloads {
    pub raw: McJavaDownloadInfo,
    pub lzma: Option<McJavaDownloadInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McJavaDownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McAssetObjects {
    pub objects: HashMap<String, McAssetObject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McAssetObject {
    pub hash: String,
    pub size: u64,
}

impl McAssetObject {
    pub fn download_url(&self) -> String {
        format!(
            "https://resources.download.minecraft.net/{}/{}",
            &self.hash[..2],
            self.hash
        )
    }
}
