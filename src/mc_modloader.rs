use quick_xml::de::from_str;
use serde::Deserialize;

// === XML 解析結構 (用於 Forge / NeoForge) ===

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

// === JSON 解析結構 (用於 Fabric) ===

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

    /// UI 顯示名稱（InstanceConfig.mod_loader）→ ModLoaderType；"None" 或未知回傳 None
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Forge" => Some(ModLoaderType::Forge),
            "NeoForge" => Some(ModLoaderType::NeoForge),
            "Fabric" => Some(ModLoaderType::Fabric),
            _ => None,
        }
    }
}

/// 共用 HTTP client：重用連線池與 TLS session，避免每次請求重新握手
fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

const FORGE_METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const NEOFORGE_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

/// 整份 maven-metadata 版本清單快取（發佈的版本不會消失，程式生命週期內快取一次即可）
type MetadataCache = std::sync::Mutex<Option<std::sync::Arc<Vec<String>>>>;
static FORGE_METADATA_CACHE: MetadataCache = std::sync::Mutex::new(None);
static NEOFORGE_METADATA_CACHE: MetadataCache = std::sync::Mutex::new(None);

/// 各 Loader 對特定 MC 版本是否有可用版本
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

pub struct ModLoaderApi;

impl ModLoaderApi {
    /// 取得特定 Loader 的所有可用版本（針對特定 MC 版本進行篩選）
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

    /// 從 Fabric Meta API 取得對應 MC 版本的 Loader 清單。
    /// 依 MC 版本快取成功結果：check_availability 與後續的版本清單抓取
    /// 會查同一端點，避免每次切換 Loader 重複請求（失敗不快取，下次重試）
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

        // Fabric Meta 對不支援的 MC 版本（如 1.12.2，無 intermediary）回 400 + 空陣列，
        // 視為「無可用版本」而非錯誤，讓 UI 顯示正確提示
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

    /// 查詢三種 Loader 是否支援指定 MC 版本；網路失敗時回傳 true（視為可用），
    /// 避免暫時性錯誤把選項鎖死
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
        // 嘗試取得 Forge promotions，失敗則靜默略過
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
                if !recommended_suffix.is_empty() && v.ends_with(&recommended_suffix) {
                    format!("{} (Recommended)", v)
                } else if !latest_suffix.is_empty() && v.ends_with(&latest_suffix) {
                    format!("{} (Latest)", v)
                } else {
                    v
                }
            })
            .collect())
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

    /// 依標記挑選預設版本索引：Recommended > Stable > Latest > 第一項
    pub fn default_version_index(versions: &[String]) -> usize {
        for marker in ["(Recommended)", "(Stable)", "(Latest)"] {
            if let Some(i) = versions.iter().position(|v| v.ends_with(marker)) {
                return i;
            }
        }
        0
    }

    /// 下載並快取整份 maven-metadata 版本清單；已快取則直接回傳
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

    /// 從快取的 maven-metadata 依前綴篩選，反轉為「最新在前」，
    /// 與 Fabric Meta API 的排序一致，讓 UI 下拉選單行為統一
    async fn get_xml_versions(
        url: &str,
        cache: &MetadataCache,
        prefix: &str,
    ) -> anyhow::Result<Vec<String>> {
        let all = Self::cached_maven_versions(url, cache).await?;

        // maven-metadata.xml 的版本列表為舊→新，篩選後反轉
        let mut filtered: Vec<String> = all
            .iter()
            .filter(|v| v.starts_with(prefix))
            .cloned()
            .collect();
        filtered.reverse();

        Ok(filtered)
    }

    /// 計算 NeoForge 對應 MC 版本的前綴字串
    fn get_neoforge_prefix(mc_version: &str) -> String {
        if mc_version.starts_with("1.") {
            let mut parts = mc_version.splitn(4, '.');
            let _ = parts.next(); // skip "1"
            let minor = parts.next().unwrap_or("0");
            let patch = parts.next().unwrap_or("0");
            format!("{}.{}.", minor, patch)
        } else {
            // 未來的 26.1 格式
            format!("{}.", mc_version)
        }
    }

    /// 執行 Mod Loader 安裝流程
    /// 回傳安裝完成後的 profile version_id（例如 "1.20.4-forge-49.0.50" or "fabric-loader-0.15.7-1.20.4"）
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

        // Bytes 為引用計數，clone 僅複製指標，避免整包 installer（數 MB）重複拷貝
        let bytes = resp.bytes().await?;
        tokio::fs::write(&installer_path, &bytes).await?;

        // 讀取 install_profile.json：偵測格式，並取得舊版的真實 version_id
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

        // 早期返回：若目標版本 JSON 已存在
        // 舊版：用 versionInfo.id；新版：用推導出的 profile_id
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
            // 舊版 Forge：不支援 --installClient，直接解析 JAR 手動安裝
            let mc_dir_owned = mc_dir.to_path_buf();
            tokio::task::spawn_blocking(move || {
                Self::install_forge_old_format_sync(&bytes, &mc_dir_owned)
            })
            .await??
        } else {
            // 新版 Forge（1.13+）：執行 installer --installClient
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

    /// 舊版 Forge installer（≤1.12）：install_profile.json 含 versionInfo，
    /// 直接從 JAR 解出版本 JSON 與內嵌的 Forge JAR，不需執行任何子程序。
    /// 回傳 versionInfo.id（即實際安裝後的版本目錄名稱）。
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

        // 寫入版本 JSON
        let version_dir = mc_dir.join("versions").join(version_id);
        std::fs::create_dir_all(&version_dir)?;
        std::fs::write(
            version_dir.join(format!("{}.json", version_id)),
            serde_json::to_string_pretty(version_info)?,
        )?;

        // 取出 installer 中內嵌的 Forge JAR（filePath 是 JAR 內部路徑，path 是 Maven 座標）
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

    /// 針對沒有 installer.jar 的早期 Forge，依序嘗試：
    ///   1. universal.zip（1.3.x 部分版本）→ 加入 libraries 陣列
    ///   2. client.zip（1.0–1.2.5）→ 解出 minecraft.jar 置為版本 JAR
    async fn install_legacy_forge_without_installer(
        mc_version: &str,
        loader_version: &str,
        mc_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let effective_id = loader_version.replacen("-", "-forge-", 1);

        // 已安裝則直接回傳
        let json_path = mc_dir
            .join("versions")
            .join(&effective_id)
            .join(format!("{}.json", &effective_id));
        if json_path.exists() {
            return Ok(effective_id);
        }

        // ── 嘗試 1：universal.zip ──────────────────────────────────
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

        // ── 嘗試 2：client.zip（1.0–1.2.5 無 universal.zip）─────────
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

    /// client.zip 安裝路徑（1.0–1.2.5）：
    /// 從 zip 中解出 minecraft.jar，置為版本 JAR；版本 JSON 的 downloads 設為空物件，
    /// 讓 install_client 偵測到 client URL 為 None 後跳過下載，保留此 JAR。
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

        // 從 zip 取出 minecraft.jar（支援根目錄或 bin/ 子目錄）
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

        // "downloads": {} → McVersionDownloads { client: None, server: None }
        // → install_client 偵測到 client = None 後跳過，不覆蓋我們放好的 JAR
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

    /// Maven 座標（`group:artifact:version[:classifier]`）→ 相對於 libraries/ 的路徑
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
        // Installer 需要 launcher_profiles.json，否則拒絕安裝
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

    /// 在 mc_dir 下預先建立 versions/<id>/<id>.json，模擬「已安裝」狀態
    fn create_fake_profile(mc_dir: &std::path::Path, profile_id: &str) -> std::path::PathBuf {
        let dir = mc_dir.join("versions").join(profile_id);
        std::fs::create_dir_all(&dir).unwrap();
        let json_path = dir.join(format!("{}.json", profile_id));
        std::fs::write(&json_path, "{}").unwrap();
        json_path
    }

    /// 建立一個記憶體中的 zip，內含指定路徑的檔案
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

    // === 純函式（同步）===

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
        assert_eq!(ModLoaderType::from_name("forge"), None); // 大小寫敏感
    }

    #[test]
    fn test_default_version_index_priority() {
        // Recommended > Stable > Latest > 第一項
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
        // 格式不完整時原樣回傳
        assert_eq!(
            ModLoaderApi::maven_coords_to_path("broken"),
            std::path::Path::new("broken")
        );
    }

    // === 安裝流程（非同步，不需網路）===

    #[tokio::test]
    async fn test_install_fabric_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "fabric-loader-0.15.7-1.20.4";
        create_fake_profile(tmp.path(), profile_id);

        // 已存在時應直接回傳，不發出任何網路請求
        let result = ModLoaderApi::install_fabric("1.20.4", "0.15.7", tmp.path())
            .await
            .unwrap();
        assert_eq!(result, profile_id);
    }

    #[tokio::test]
    async fn test_install_neoforge_skips_when_profile_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_id = "neoforge-20.4.237";
        create_fake_profile(tmp.path(), profile_id);

        // 已存在時應直接回傳，不下載 installer、不執行 Java
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
        // loader_version "1.4.7-6.6.2.534" → effective_id "1.4.7-forge-6.6.2.534"
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
        // UI 傳入的版本字串帶有 " (Stable)" 標記，install_modloader 應先清理
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

        // 版本 JAR 應為 zip 內的 minecraft.jar
        let jar_path = tmp
            .path()
            .join("versions")
            .join(effective_id)
            .join(format!("{}.jar", effective_id));
        assert_eq!(std::fs::read(&jar_path).unwrap(), jar_content);

        // 版本 JSON：inheritsFrom 正確、downloads 為空物件（避免覆蓋 JAR）
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

    // === 版本清單 API（需網路）===

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
        // Fabric 不支援 1.12.2（Meta API 回 400）→ 應回傳空清單而非錯誤
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
}
