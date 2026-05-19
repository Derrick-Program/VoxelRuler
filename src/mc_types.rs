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
    pub compliance_level: i32,
    pub minimum_launcher_version: i32,
    pub main_class: String,
    pub java_version: McJavaVersion,
    pub downloads: McVersionDownloads,
    pub asset_index: McAssetIndex,
    pub assets: String,
    pub logging: HashMap<String, McLoggingConfig>,
    pub libraries: Vec<McLibrary>,
    pub arguments: McArguments,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McJavaVersion {
    pub component: String,
    pub major_version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McVersionDownloads {
    pub client: McArtifactInfo,
    pub server: McArtifactInfo,
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
    pub downloads: McLibraryDownloads,
    pub rules: Option<Vec<McRule>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McLibraryDownloads {
    pub artifact: McArtifactInfo,
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
    pub name: Option<McRuleOS>, // "windows", "osx", "linux"
    pub arch: Option<McRuleArch>,   // "x86"
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
    pub default_user_jvm: Vec<McArgumentItem>,
    pub game: Vec<McArgumentItem>,
    pub jvm: Vec<McArgumentItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McArgumentItem {
    /// 純字串參數 (例如 "--username")
    Simple(String),
    /// 帶有篩選規則的參數物件
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
