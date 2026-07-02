use std::path::PathBuf;

pub struct McPaths {
    base: PathBuf,
}

impl McPaths {
    pub fn new() -> anyhow::Result<Self> {
        let base = crate::PROJECT_DIR
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to get system app directory"))?
            .data_dir()
            .to_path_buf();
        Ok(Self { base })
    }

    /// 確保目錄存在，若建立失敗則記錄警告，避免錯誤被完全遮蔽 (Robustness)
    fn ensure_dir(path: PathBuf) -> PathBuf {
        if !path.exists()
            && let Err(e) = std::fs::create_dir_all(&path)
        {
            tracing::warn!(path = %path.display(), error = %e, "Failed to create directory");
        }
        path
    }

    pub fn base_dir(&self) -> PathBuf {
        self.base.clone()
    }

    pub fn versions_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("versions"))
    }

    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        Self::ensure_dir(self.versions_dir().join(version_id))
    }

    pub fn version_jar(&self, version_id: &str) -> PathBuf {
        let d = self
            .version_dir(version_id)
            .join(format!("{}.jar", version_id));
        if !d.exists()
            && let Err(e) = std::fs::File::create(&d)
        {
            tracing::warn!(path = %d.display(), error = %e, "Failed to create version jar");
        }
        d
    }

    pub fn libraries_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("libraries"))
    }

    pub fn assets_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("assets"))
    }

    pub fn asset_indexes_dir(&self) -> PathBuf {
        Self::ensure_dir(self.assets_dir().join("indexes"))
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        Self::ensure_dir(self.assets_dir().join("objects"))
    }

    pub fn java_dir(&self, component: &str) -> PathBuf {
        Self::ensure_dir(self.base.join("java").join(component))
    }

    pub fn java_bin(&self, component: &str) -> PathBuf {
        #[cfg(target_os = "windows")]
        return self.java_dir(component).join("bin").join("javaw.exe");
        #[cfg(target_os = "linux")]
        return self.java_dir(component).join("bin").join("java");
        #[cfg(target_os = "macos")]
        return self
            .java_dir(component)
            .join("jre.bundle")
            .join("Contents")
            .join("Home")
            .join("bin")
            .join("java");
    }

    pub fn instances_base_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("instances"))
    }

    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        Self::ensure_dir(self.base.join("instances").join(instance_id))
    }

    pub fn skins_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("skins"))
    }

    pub fn skins_history_file(&self) -> PathBuf {
        self.skins_dir().join("index.json")
    }

    pub fn capes_dir(&self) -> PathBuf {
        Self::ensure_dir(self.base.join("capes"))
    }

    pub fn natives_dir(&self, version_id: &str) -> PathBuf {
        Self::ensure_dir(self.version_dir(version_id).join("natives"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Build an McPaths rooted at a temporary directory without relying on PROJECT_DIR.
    fn paths_in(dir: &TempDir) -> McPaths {
        McPaths {
            base: dir.path().to_path_buf(),
        }
    }

    #[test]
    fn test_base_dir_matches_root() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        assert_eq!(p.base_dir(), dir.path());
    }

    #[test]
    fn test_versions_dir_created_under_base() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let versions = p.versions_dir();
        assert!(versions.exists());
        assert_eq!(versions, dir.path().join("versions"));
    }

    #[test]
    fn test_version_dir_is_nested_under_versions() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let vd = p.version_dir("1.21.1");
        assert!(vd.exists());
        assert_eq!(vd, dir.path().join("versions").join("1.21.1"));
    }

    #[test]
    fn test_version_jar_path_has_correct_name() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let jar = p.version_jar("1.21.1");
        assert_eq!(jar.file_name().unwrap(), "1.21.1.jar");
        assert!(jar.exists());
    }

    #[test]
    fn test_libraries_dir_created() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let libs = p.libraries_dir();
        assert!(libs.exists());
        assert_eq!(libs, dir.path().join("libraries"));
    }

    #[test]
    fn test_assets_dir_created() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        assert!(p.assets_dir().exists());
        assert!(p.asset_indexes_dir().exists());
        assert!(p.asset_objects_dir().exists());
    }

    #[test]
    fn test_asset_indexes_is_under_assets() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        assert_eq!(
            p.asset_indexes_dir(),
            dir.path().join("assets").join("indexes")
        );
    }

    #[test]
    fn test_java_dir_uses_component_name() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let jd = p.java_dir("java-runtime-delta");
        assert!(jd.exists());
        assert_eq!(jd, dir.path().join("java").join("java-runtime-delta"));
    }

    #[test]
    fn test_instances_base_dir_created() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let inst = p.instances_base_dir();
        assert!(inst.exists());
        assert_eq!(inst, dir.path().join("instances"));
    }

    #[test]
    fn test_instance_dir_is_namespaced() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let id = p.instance_dir("my-world");
        assert!(id.exists());
        assert_eq!(id, dir.path().join("instances").join("my-world"));
    }

    #[test]
    fn test_skins_history_file_under_skins() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        assert_eq!(
            p.skins_history_file(),
            dir.path().join("skins").join("index.json")
        );
    }

    #[test]
    fn test_capes_dir_created() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let capes = p.capes_dir();
        assert!(capes.exists());
        assert_eq!(capes, dir.path().join("capes"));
    }

    #[test]
    fn test_natives_dir_under_version() {
        let dir = TempDir::new().unwrap();
        let p = paths_in(&dir);
        let nat = p.natives_dir("1.12.2");
        assert!(nat.exists());
        assert_eq!(
            nat,
            dir.path().join("versions").join("1.12.2").join("natives")
        );
    }
}
