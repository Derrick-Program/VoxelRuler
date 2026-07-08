use quick_xml::de::from_str;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct MavenMetadata {
    versioning: Versioning,
}

#[derive(Debug, Deserialize)]
struct Versioning {
    versions: Versions,
}

#[derive(Debug, Deserialize)]
struct Versions {
    #[serde(rename = "version", default)]
    list: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FabricLoaderEntry {
    pub loader: FabricLoaderInfo,
}

#[derive(Debug, Deserialize)]
pub struct FabricLoaderInfo {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLoaderType {
    Forge,
    NeoForge,
    Fabric,
}

impl ModLoaderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModLoaderType::Forge => "forge",
            ModLoaderType::NeoForge => "neoforge",
            ModLoaderType::Fabric => "fabric",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Forge" => Some(ModLoaderType::Forge),
            "NeoForge" => Some(ModLoaderType::NeoForge),
            "Fabric" => Some(ModLoaderType::Fabric),
            _ => None,
        }
    }
}

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

const FORGE_METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const NEOFORGE_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

type MetadataCache = std::sync::Mutex<Option<std::sync::Arc<Vec<String>>>>;
static FORGE_METADATA_CACHE: MetadataCache = std::sync::Mutex::new(None);
static NEOFORGE_METADATA_CACHE: MetadataCache = std::sync::Mutex::new(None);

#[derive(Debug, Clone, Copy)]
pub struct LoaderAvailability {
    pub fabric: bool,
    pub forge: bool,
    pub neoforge: bool,
}

impl LoaderAvailability {
    pub fn supports(&self, loader_type: ModLoaderType) -> bool {
        match loader_type {
            ModLoaderType::Forge => self.forge,
            ModLoaderType::NeoForge => self.neoforge,
            ModLoaderType::Fabric => self.fabric,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoaderFetchResult {
    pub availability: LoaderAvailability,
    pub versions: Vec<String>,
    pub error: Option<String>,
}

pub struct ModLoaderApi;

impl ModLoaderApi {
    pub async fn get_loader_versions(
        loader_type: ModLoaderType,
        mc_version: &str,
    ) -> anyhow::Result<Vec<String>> {
        match loader_type {
            ModLoaderType::Fabric => Self::get_fabric_versions(mc_version).await,
            ModLoaderType::NeoForge => Self::get_neoforge_versions(mc_version).await,
            ModLoaderType::Forge => Self::get_forge_versions(mc_version).await,
        }
    }

    pub async fn fetch_loader_state(
        mc_version: &str,
        loader: Option<ModLoaderType>,
    ) -> LoaderFetchResult {
        let availability = Self::check_availability(mc_version).await;

        let (versions, error) = match loader {
            Some(lt) if availability.supports(lt) => {
                match Self::get_loader_versions(lt, mc_version).await {
                    Ok(v) => (v, None),
                    Err(e) => (Vec::new(), Some(e.to_string())),
                }
            }
            _ => (Vec::new(), None),
        };

        LoaderFetchResult {
            availability,
            versions,
            error,
        }
    }

    async fn get_fabric_versions(mc_version: &str) -> anyhow::Result<Vec<String>> {
        static CACHE: std::sync::Mutex<
            std::collections::BTreeMap<String, std::sync::Arc<Vec<String>>>,
        > = std::sync::Mutex::new(std::collections::BTreeMap::new());

        if let Some(list) = CACHE.lock().unwrap().get(mc_version) {
            return Ok(list.as_ref().clone());
        }

        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}",
            mc_version
        );
        let resp = http().get(&url).send().await?;

        let versions: Vec<String> = if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            Vec::new()
        } else {
            let entries: Vec<FabricLoaderEntry> = resp.error_for_status()?.json().await?;
            entries
                .into_iter()
                .map(|e| {
                    if e.loader.stable {
                        format!("{} (Stable)", e.loader.version)
                    } else {
                        format!("{} (Beta)", e.loader.version)
                    }
                })
                .collect()
        };

        CACHE.lock().unwrap().insert(
            mc_version.to_string(),
            std::sync::Arc::new(versions.clone()),
        );
        Ok(versions)
    }

    pub async fn check_availability(mc_version: &str) -> LoaderAvailability {
        let forge_prefix = format!("{}-", mc_version);
        let neo_prefix = Self::get_neoforge_prefix(mc_version);
        let (fabric, forge, neoforge) = tokio::join!(
            Self::get_fabric_versions(mc_version),
            Self::cached_maven_versions(FORGE_METADATA_URL, &FORGE_METADATA_CACHE),
            Self::cached_maven_versions(NEOFORGE_METADATA_URL, &NEOFORGE_METADATA_CACHE),
        );
        LoaderAvailability {
            fabric: fabric.map(|v| !v.is_empty()).unwrap_or(true),
            forge: forge
                .map(|list| list.iter().any(|v| v.starts_with(&forge_prefix)))
                .unwrap_or(true),
            neoforge: neoforge
                .map(|list| list.iter().any(|v| v.starts_with(&neo_prefix)))
                .unwrap_or(true),
        }
    }

    async fn get_forge_versions(mc_version: &str) -> anyhow::Result<Vec<String>> {
        let versions = Self::get_xml_versions(
            FORGE_METADATA_URL,
            &FORGE_METADATA_CACHE,
            &format!("{}-", mc_version),
        )
        .await?;

        let mut latest_suffix = String::new();
        let mut recommended_suffix = String::new();
        if let Ok(json) = async {
            let res = http()
                .get("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
                .send()
                .await?;
            res.json::<serde_json::Value>().await
        }.await
            && let Some(promos) = json.get("promos").and_then(|p| p.as_object()) {
                if let Some(l) = promos
                    .get(&format!("{}-latest", mc_version))
                    .and_then(|v| v.as_str())
                {
                    latest_suffix = l.to_string();
                }
                if let Some(r) = promos
                    .get(&format!("{}-recommended", mc_version))
                    .and_then(|v| v.as_str())
                {
                    recommended_suffix = r.to_string();
                }
            }

        Ok(versions
            .into_iter()
            .map(|v| {
                if Self::matches_promo(&v, mc_version, &recommended_suffix) {
                    format!("{} (Recommended)", v)
                } else if Self::matches_promo(&v, mc_version, &latest_suffix) {
                    format!("{} (Latest)", v)
                } else {
                    v
                }
            })
            .collect())
    }

    fn matches_promo(version: &str, mc_version: &str, promo: &str) -> bool {
        if promo.is_empty() {
            return false;
        }
        let base = format!("{}-{}", mc_version, promo);
        version == base || version.starts_with(&format!("{}-", base))
    }

    async fn get_neoforge_versions(mc_version: &str) -> anyhow::Result<Vec<String>> {
        let versions = Self::get_xml_versions(
            NEOFORGE_METADATA_URL,
            &NEOFORGE_METADATA_CACHE,
            &Self::get_neoforge_prefix(mc_version),
        )
        .await?;

        Ok(versions
            .into_iter()
            .map(|v| {
                if v.contains("-beta") {
                    format!("{} (Beta)", v)
                } else {
                    format!("{} (Stable)", v)
                }
            })
            .collect())
    }

    pub fn default_version_index(versions: &[String]) -> usize {
        for marker in ["(Recommended)", "(Stable)", "(Latest)"] {
            if let Some(i) = versions.iter().position(|v| v.ends_with(marker)) {
                return i;
            }
        }
        0
    }

    async fn cached_maven_versions(
        url: &str,
        cache: &MetadataCache,
    ) -> anyhow::Result<std::sync::Arc<Vec<String>>> {
        if let Some(list) = cache.lock().unwrap().clone() {
            return Ok(list);
        }

        let xml_str = http()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let metadata: MavenMetadata = from_str(&xml_str)?;
        let list = std::sync::Arc::new(metadata.versioning.versions.list);
        *cache.lock().unwrap() = Some(std::sync::Arc::clone(&list));
        Ok(list)
    }

    async fn get_xml_versions(
        url: &str,
        cache: &MetadataCache,
        prefix: &str,
    ) -> anyhow::Result<Vec<String>> {
        let all = Self::cached_maven_versions(url, cache).await?;

        let mut filtered: Vec<String> = all
            .iter()
            .filter(|v| v.starts_with(prefix))
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            Self::version_sort_key(&b[prefix.len()..])
                .cmp(&Self::version_sort_key(&a[prefix.len()..]))
        });

        Ok(filtered)
    }

    fn version_sort_key(s: &str) -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u64>().unwrap_or(u64::MAX))
            .collect()
    }

    fn get_neoforge_prefix(mc_version: &str) -> String {
        if mc_version.starts_with("1.") {
            let mut parts = mc_version.splitn(4, '.');
            let _ = parts.next();
            let minor = parts.next().unwrap_or("0");
            let patch = parts.next().unwrap_or("0");
            format!("{}.{}.", minor, patch)
        } else {
            format!("{}.", mc_version)
        }
    }

    pub async fn install_modloader(
        loader_type: ModLoaderType,
        mc_version: &str,
        loader_version: &str,
        java_path: &std::path::Path,
        minecraft_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let clean_version = loader_version
            .split_whitespace()
            .next()
            .unwrap_or(loader_version);

        match loader_type {
            ModLoaderType::Fabric => {
                Self::install_fabric(mc_version, clean_version, minecraft_dir).await
            }
            ModLoaderType::Forge => {
                Self::install_forge(mc_version, clean_version, java_path, minecraft_dir).await
            }
            ModLoaderType::NeoForge => {
                Self::install_neoforge(mc_version, clean_version, java_path, minecraft_dir).await
            }
        }
    }

    async fn install_fabric(
        mc_version: &str,
        loader_version: &str,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let profile_id = format!("fabric-loader-{}-{}", loader_version, mc_version);
        let target_dir = mc_dir.join("versions").join(&profile_id);
        let json_path = target_dir.join(format!("{}.json", profile_id));

        if json_path.exists() {
            return Ok(profile_id);
        }

        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            mc_version, loader_version
        );

        let client = reqwest::Client::new();
        let json_str = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        tokio::fs::create_dir_all(&target_dir).await?;
        tokio::fs::write(&json_path, json_str).await?;

        Ok(profile_id)
    }

    async fn install_forge(
        mc_version: &str,
        loader_version: &str,
        java_path: &std::path::Path,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let guessed_id = loader_version.replacen("-", "-forge-", 1);
        let guessed_json = mc_dir
            .join("versions")
            .join(&guessed_id)
            .join(format!("{}.json", guessed_id));
        if guessed_json.exists() {
            return Ok(guessed_id);
        }

        let url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
            loader_version, loader_version
        );

        let temp_dir = std::env::temp_dir();
        let installer_path = temp_dir.join(format!("installer-{}.jar", uuid::Uuid::new_v4()));

        let resp = match http().get(&url).send().await {
            Ok(r) => {
                if r.status() == reqwest::StatusCode::NOT_FOUND {
                    tracing::warn!(
                        "Forge installer 404 Not Found. Attempting to process as early version without installer (e.g. 1.4.x)..."
                    );
                    return Self::install_legacy_forge_without_installer(
                        mc_version,
                        loader_version,
                        mc_dir,
                    )
                    .await;
                }
                r.error_for_status()?
            }
            Err(e) => return Err(e.into()),
        };

        let bytes = resp.bytes().await?;
        tokio::fs::write(&installer_path, &bytes).await?;

        let bytes_for_detect = bytes.clone();
        let (is_old_format, old_version_id) =
            tokio::task::spawn_blocking(move || -> anyhow::Result<(bool, Option<String>)> {
                use std::io::Read;
                let cursor = std::io::Cursor::new(&bytes_for_detect);
                let mut zip = zip::ZipArchive::new(cursor)?;
                let mut file = zip.by_name("install_profile.json")?;
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                let profile: serde_json::Value = serde_json::from_str(&content)?;
                if let Some(vi) = profile.get("versionInfo") {
                    let id = vi.get("id").and_then(|v| v.as_str()).map(str::to_owned);
                    Ok((true, id))
                } else {
                    Ok((false, None))
                }
            })
            .await??;

        let effective_id = if is_old_format {
            old_version_id
                .clone()
                .unwrap_or_else(|| loader_version.replacen("-", "-forge-", 1))
        } else {
            loader_version.replacen("-", "-forge-", 1)
        };
        let json_path = mc_dir
            .join("versions")
            .join(&effective_id)
            .join(format!("{}.json", effective_id));
        if json_path.exists() {
            let _ = tokio::fs::remove_file(&installer_path).await;
            return Ok(effective_id);
        }

        let result = if is_old_format {
            let mc_dir_owned = mc_dir.to_path_buf();
            tokio::task::spawn_blocking(move || {
                Self::install_forge_old_format_sync(&bytes, &mc_dir_owned)
            })
            .await??
        } else {
            let profiles_json = mc_dir.join("launcher_profiles.json");
            if !profiles_json.exists() {
                tokio::fs::write(&profiles_json, "{}").await?;
            }

            let java_path_owned = java_path.to_path_buf();
            let mc_dir_owned = mc_dir.to_path_buf();
            let installer_path_owned = installer_path.clone();

            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(java_path_owned)
                    .current_dir(&mc_dir_owned)
                    .arg("-jar")
                    .arg(&installer_path_owned)
                    .arg("--installClient")
                    .arg(&mc_dir_owned)
                    .output()
            })
            .await??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                anyhow::bail!("Forge installation failed: {}\n{}", stderr, stdout);
            }
            loader_version.replacen("-", "-forge-", 1)
        };

        let _ = tokio::fs::remove_file(&installer_path).await;
        Ok(result)
    }

    fn install_forge_old_format_sync(
        bytes: &[u8],
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        use std::io::Read;

        let cursor = std::io::Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor)?;

        let profile: serde_json::Value = {
            let mut file = zip.by_name("install_profile.json")?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            serde_json::from_str(&content)?
        };

        let version_info = profile
            .get("versionInfo")
            .ok_or_else(|| anyhow::anyhow!("install_profile.json missing versionInfo"))?;

        let version_id = version_info
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("versionInfo missing id"))?;

        let version_dir = mc_dir.join("versions").join(version_id);
        std::fs::create_dir_all(&version_dir)?;
        std::fs::write(
            version_dir.join(format!("{}.json", version_id)),
            serde_json::to_string_pretty(version_info)?,
        )?;

        let embedded_jar = profile.get("install").and_then(|inst| {
            let file_path = inst.get("filePath").and_then(|v| v.as_str())?;
            let maven_coords = inst.get("path").and_then(|v| v.as_str())?;
            Some((file_path, maven_coords))
        });
        if let Some((file_path, maven_coords)) = embedded_jar
            && let Ok(mut entry) = zip.by_name(file_path)
        {
            let lib_path = mc_dir
                .join("libraries")
                .join(Self::maven_coords_to_path(maven_coords));
            if let Some(parent) = lib_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut jar_bytes = Vec::new();
            entry.read_to_end(&mut jar_bytes)?;
            std::fs::write(&lib_path, jar_bytes)?;
        }

        Ok(version_id.to_owned())
    }

    async fn install_legacy_forge_without_installer(
        mc_version: &str,
        loader_version: &str,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let effective_id = loader_version.replacen("-", "-forge-", 1);

        let json_path = mc_dir
            .join("versions")
            .join(&effective_id)
            .join(format!("{}.json", &effective_id));
        if json_path.exists() {
            return Ok(effective_id);
        }

        let universal_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{0}/forge-{0}-universal.zip",
            loader_version
        );
        tracing::info!(
            "Attempting to download Forge universal.zip: {}",
            universal_url
        );
        let resp = http().get(&universal_url).send().await?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await?;

            use sha1::{Digest, Sha1};
            let sha1 = Sha1::digest(&bytes)
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            let size = bytes.len() as u64;

            let version_dir = mc_dir.join("versions").join(&effective_id);
            tokio::fs::create_dir_all(&version_dir).await?;

            let rel_path = format!(
                "net/minecraftforge/forge/{0}/forge-{0}-universal.zip",
                loader_version
            );
            let lib_path = mc_dir.join("libraries").join(&rel_path);
            if let Some(parent) = lib_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&lib_path, &bytes).await?;

            let profile = serde_json::json!({
                "id": effective_id,
                "inheritsFrom": mc_version,
                "time": "2012-01-01T00:00:00+00:00",
                "releaseTime": "2012-01-01T00:00:00+00:00",
                "type": "release",
                "mainClass": "net.minecraft.client.Minecraft",
                "minecraftArguments": "${auth_player_name} ${auth_session} --gameDir ${game_directory} --assetsDir ${game_assets}",
                "libraries": [{
                    "name": format!("net.minecraftforge:forge:{}:universal", loader_version),
                    "downloads": {
                        "artifact": {
                            "path": rel_path,
                            "url": universal_url,
                            "sha1": sha1,
                            "size": size
                        }
                    }
                }]
            });

            tokio::fs::write(&json_path, serde_json::to_string_pretty(&profile)?).await?;
            return Ok(effective_id);
        }

        let client_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{0}/forge-{0}-client.zip",
            loader_version
        );
        tracing::info!(
            "universal.zip does not exist, trying client.zip: {}",
            client_url
        );
        let resp = http().get(&client_url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "Forge {} has no available installation package (tried installer.jar / universal.zip / client.zip)",
                loader_version
            );
        }
        let bytes = resp.error_for_status()?.bytes().await?;
        Self::install_legacy_client_zip(
            mc_version,
            loader_version,
            mc_dir,
            &effective_id,
            &json_path,
            &bytes,
        )
        .await
    }

    async fn install_legacy_client_zip(
        mc_version: &str,
        loader_version: &str,
        mc_dir: &std::path::Path,
        effective_id: &str,
        json_path: &std::path::Path,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        let version_dir = mc_dir.join("versions").join(effective_id);
        tokio::fs::create_dir_all(&version_dir).await?;

        let bytes_owned = bytes.to_vec();
        let jar_bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
            use std::io::Read;
            let cursor = std::io::Cursor::new(bytes_owned);
            let mut zip = zip::ZipArchive::new(cursor)?;
            for candidate in &["minecraft.jar", "bin/minecraft.jar"] {
                if let Ok(mut entry) = zip.by_name(candidate) {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
            }
            anyhow::bail!("Could not find minecraft.jar in client.zip")
        })
        .await??;

        let jar_path = version_dir.join(format!("{}.jar", effective_id));
        tokio::fs::write(&jar_path, jar_bytes).await?;

        // "downloads": {} makes install_client treat this version as having no client download and skip overwriting the JAR written above
        let profile = serde_json::json!({
            "id": effective_id,
            "inheritsFrom": mc_version,
            "time": "2012-01-01T00:00:00+00:00",
            "releaseTime": "2012-01-01T00:00:00+00:00",
            "type": "release",
            "mainClass": "net.minecraft.client.Minecraft",
            "minecraftArguments": "${auth_player_name} ${auth_session} --gameDir ${game_directory} --assetsDir ${game_assets}",
            "downloads": {},
            "libraries": []
        });

        tokio::fs::write(json_path, serde_json::to_string_pretty(&profile)?).await?;
        Ok(effective_id.to_owned())
    }

    fn maven_coords_to_path(coords: &str) -> std::path::PathBuf {
        let mut parts = coords.splitn(4, ':');
        let (Some(group), Some(artifact), Some(version)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return std::path::PathBuf::from(coords);
        };
        let classifier = parts.next();
        let base: std::path::PathBuf = group.split('.').collect();
        let filename = match classifier {
            Some(cls) => format!("{artifact}-{version}-{cls}.jar"),
            None => format!("{artifact}-{version}.jar"),
        };
        base.join(artifact).join(version).join(filename)
    }

    async fn install_neoforge(
        _mc_version: &str,
        loader_version: &str,
        java_path: &std::path::Path,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let profile_id = format!("neoforge-{}", loader_version);
        let json_path = mc_dir
            .join("versions")
            .join(&profile_id)
            .join(format!("{}.json", profile_id));

        if json_path.exists() {
            return Ok(profile_id);
        }

        let url = format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
            loader_version, loader_version
        );

        Self::run_installer_jar(&url, java_path, mc_dir).await?;

        Ok(profile_id)
    }

    async fn run_installer_jar(
        url: &str,
        java_path: &std::path::Path,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<()> {
        let profiles_json = mc_dir.join("launcher_profiles.json");
        if !profiles_json.exists() {
            tokio::fs::write(&profiles_json, "{}").await?;
        }

        let temp_dir = std::env::temp_dir();
        let installer_path = temp_dir.join(format!("installer-{}.jar", uuid::Uuid::new_v4()));

        let bytes = http()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        tokio::fs::write(&installer_path, bytes).await?;

        let java_path_owned = java_path.to_path_buf();
        let mc_dir_owned = mc_dir.to_path_buf();
        let installer_path_owned = installer_path.clone();

        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new(java_path_owned)
                .current_dir(&mc_dir_owned)
                .arg("-jar")
                .arg(&installer_path_owned)
                .arg("--installClient")
                .arg(&mc_dir_owned)
                .output()
        })
        .await??;

        let _ = tokio::fs::remove_file(&installer_path).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!("Installation failed: {}\n{}", stderr, stdout);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_fake_profile(mc_dir: &std::path::Path, profile_id: &str) -> std::path::PathBuf {
        let dir = mc_dir.join("versions").join(profile_id);
        std::fs::create_dir_all(&dir).unwrap();
        let json_path = dir.join(format!("{}.json", profile_id));
        std::fs::write(&json_path, "{}").unwrap();
        json_path
    }

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default();
            for (name, data) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn test_from_name() {
        assert_eq!(
            ModLoaderType::from_name("Forge"),
            Some(ModLoaderType::Forge)
        );
        assert_eq!(
            ModLoaderType::from_name("NeoForge"),
            Some(ModLoaderType::NeoForge)
        );
        assert_eq!(
            ModLoaderType::from_name("Fabric"),
            Some(ModLoaderType::Fabric)
        );
        assert_eq!(ModLoaderType::from_name("None"), None);
        assert_eq!(ModLoaderType::from_name(""), None);
        assert_eq!(ModLoaderType::from_name("forge"), None);
    }

    #[test]
    fn test_default_version_index_priority() {
        let versions = vec![
            "49.0.50".to_string(),
            "49.0.30 (Latest)".to_string(),
            "49.0.10 (Stable)".to_string(),
            "49.0.3 (Recommended)".to_string(),
        ];
        assert_eq!(ModLoaderApi::default_version_index(&versions), 3);

        let versions = vec!["0.15.7 (Beta)".to_string(), "0.15.6 (Stable)".to_string()];
        assert_eq!(ModLoaderApi::default_version_index(&versions), 1);

        let versions = vec!["a".to_string(), "b".to_string()];
        assert_eq!(ModLoaderApi::default_version_index(&versions), 0);
    }

    #[test]
    fn test_matches_promo() {
        assert!(ModLoaderApi::matches_promo(
            "1.20.1-47.4.10",
            "1.20.1",
            "47.4.10"
        ));
        assert!(ModLoaderApi::matches_promo(
            "1.7.10-10.13.4.1614-1.7.10",
            "1.7.10",
            "10.13.4.1614"
        ));
        assert!(!ModLoaderApi::matches_promo(
            "1.20.1-47.4.10",
            "1.20.1",
            "47.4.1"
        ));
        assert!(!ModLoaderApi::matches_promo("1.20.1-47.4.10", "1.20.1", ""));
    }

    #[test]
    fn test_version_sort_key_ordering() {
        assert!(
            ModLoaderApi::version_sort_key("47.4.10") > ModLoaderApi::version_sort_key("47.4.9")
        );
        assert!(
            ModLoaderApi::version_sort_key("10.13.4.1614-1.7.10")
                > ModLoaderApi::version_sort_key("10.13.2.1230-1.7.10")
        );
    }

    #[test]
    fn test_get_neoforge_prefix() {
        assert_eq!(ModLoaderApi::get_neoforge_prefix("1.20.4"), "20.4.");
        assert_eq!(ModLoaderApi::get_neoforge_prefix("1.21"), "21.0.");
        assert_eq!(ModLoaderApi::get_neoforge_prefix("26.1"), "26.1.");
    }

    #[test]
    fn test_maven_coords_to_path() {
        assert_eq!(
            ModLoaderApi::maven_coords_to_path("net.minecraftforge:forge:1.12.2-14.23.5.2859"),
            std::path::Path::new(
                "net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859.jar"
            )
        );
        assert_eq!(
            ModLoaderApi::maven_coords_to_path("org.lwjgl:lwjgl:3.2.3:natives-macos"),
            std::path::Path::new("org/lwjgl/lwjgl/3.2.3/lwjgl-3.2.3-natives-macos.jar")
        );
        assert_eq!(
            ModLoaderApi::maven_coords_to_path("broken"),
            std::path::Path::new("broken")
        );
    }

    #[tokio::test]
    async fn test_install_fabric_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "fabric-loader-0.15.7-1.20.4";
        create_fake_profile(tmp.path(), profile_id);

        let result = ModLoaderApi::install_fabric("1.20.4", "0.15.7", tmp.path())
            .await
            .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_forge_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "1.20.1-forge-47.2.0";
        create_fake_profile(tmp.path(), profile_id);

        let result = ModLoaderApi::install_forge(
            "1.20.1",
            "1.20.1-47.2.0",
            std::path::Path::new("/nonexistent/java"),
            tmp.path(),
        )
        .await
        .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_neoforge_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "neoforge-20.4.237";
        create_fake_profile(tmp.path(), profile_id);

        let result = ModLoaderApi::install_neoforge(
            "1.20.4",
            "20.4.237",
            std::path::Path::new("/nonexistent/java"),
            tmp.path(),
        )
        .await
        .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_legacy_forge_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "1.4.7-forge-6.6.2.534";
        create_fake_profile(tmp.path(), profile_id);

        let result = ModLoaderApi::install_legacy_forge_without_installer(
            "1.4.7",
            "1.4.7-6.6.2.534",
            tmp.path(),
        )
        .await
        .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_modloader_strips_display_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "fabric-loader-0.15.7-1.20.4";
        create_fake_profile(tmp.path(), profile_id);

        let result = ModLoaderApi::install_modloader(
            ModLoaderType::Fabric,
            "1.20.4",
            "0.15.7 (Stable)",
            std::path::Path::new("/nonexistent/java"),
            tmp.path(),
        )
        .await
        .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_legacy_client_zip_extracts_minecraft_jar() {
        let tmp = tempfile::tempdir().unwrap();
        let effective_id = "1.2.5-forge-3.4.9.171";
        let json_path = tmp
            .path()
            .join("versions")
            .join(effective_id)
            .join(format!("{}.json", effective_id));

        let jar_content = b"fake-jar-bytes";
        let zip_bytes = build_zip(&[("bin/minecraft.jar", jar_content.as_slice())]);

        let result = ModLoaderApi::install_legacy_client_zip(
            "1.2.5",
            "1.2.5-3.4.9.171",
            tmp.path(),
            effective_id,
            &json_path,
            &zip_bytes,
        )
        .await
        .unwrap();
        assert_eq!(result, effective_id);

        let jar_path = tmp
            .path()
            .join("versions")
            .join(effective_id)
            .join(format!("{}.jar", effective_id));
        assert_eq!(std::fs::read(&jar_path).unwrap(), jar_content);

        let profile: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(profile["id"], effective_id);
        assert_eq!(profile["inheritsFrom"], "1.2.5");
        assert!(profile["downloads"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_install_legacy_client_zip_rejects_zip_without_jar() {
        let tmp = tempfile::tempdir().unwrap();
        let effective_id = "1.2.5-forge-3.4.9.171";
        let json_path = tmp
            .path()
            .join("versions")
            .join(effective_id)
            .join(format!("{}.json", effective_id));

        let zip_bytes = build_zip(&[("readme.txt", b"nothing here".as_slice())]);

        let err = ModLoaderApi::install_legacy_client_zip(
            "1.2.5",
            "1.2.5-3.4.9.171",
            tmp.path(),
            effective_id,
            &json_path,
            &zip_bytes,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("minecraft.jar"));
    }

    #[tokio::test]
    async fn test_get_fabric_versions() {
        let versions = ModLoaderApi::get_loader_versions(ModLoaderType::Fabric, "1.20.4")
            .await
            .unwrap();
        assert!(!versions.is_empty());
        println!("Fabric 1.20.4 versions: {:?}", versions);
    }

    #[tokio::test]
    async fn test_get_fabric_versions_unsupported_mc_returns_empty() {
        let versions = ModLoaderApi::get_loader_versions(ModLoaderType::Fabric, "1.12.2")
            .await
            .unwrap();
        assert!(versions.is_empty());
    }

    #[tokio::test]
    async fn test_get_neoforge_versions() {
        let versions = ModLoaderApi::get_loader_versions(ModLoaderType::NeoForge, "1.20.4")
            .await
            .unwrap();
        assert!(!versions.is_empty());
        println!("NeoForge 1.20.4 versions: {:?}", versions);
    }

    #[tokio::test]
    async fn test_get_forge_versions() {
        let versions = ModLoaderApi::get_loader_versions(ModLoaderType::Forge, "1.20.4")
            .await
            .unwrap();
        assert!(!versions.is_empty());
        println!("Forge 1.20.4 versions: {:?}", versions);
    }

    #[tokio::test]
    async fn test_fetch_loader_state_returns_versions_for_available_loader() {
        let result = ModLoaderApi::fetch_loader_state("1.20.4", Some(ModLoaderType::Fabric)).await;
        assert!(result.availability.fabric);
        assert!(!result.versions.is_empty());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_fetch_loader_state_none_loader_returns_no_versions() {
        let result = ModLoaderApi::fetch_loader_state("1.20.4", None).await;
        assert!(result.versions.is_empty());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_fetch_loader_state_unsupported_mc_returns_empty_versions_no_error() {
        let result = ModLoaderApi::fetch_loader_state("1.12.2", Some(ModLoaderType::Fabric)).await;
        assert!(result.versions.is_empty());
        assert!(result.error.is_none());
    }
}
