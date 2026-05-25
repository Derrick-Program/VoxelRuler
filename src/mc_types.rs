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

#[derive(Debug, Serialize, Deserialize)]
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