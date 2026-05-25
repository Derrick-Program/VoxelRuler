use std::path::PathBuf;

pub struct McPaths {
    base: PathBuf,
}

impl McPaths {
    pub fn new() -> anyhow::Result<Self> {
        let base = crate::PROJECT_DIR
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("無法取得系統應用程式目錄"))?
            .data_dir()
            .to_path_buf();
        Ok(Self { base })
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.base.join("versions")
    }

    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        self.versions_dir().join(version_id)
    }

    pub fn version_jar(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id).join(format!("{}.jar", version_id))
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.base.join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.base.join("assets")
    }

    pub fn asset_indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    pub fn java_dir(&self, component: &str) -> PathBuf {
        self.base.join("java").join(component)
    }

    pub fn java_bin(&self, component: &str) -> PathBuf {
        #[cfg(target_os = "windows")]
        return self.java_dir(component).join("bin").join("javaw.exe");
        #[cfg(not(target_os = "windows"))]
        return self.java_dir(component).join("bin").join("java");
    }

    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.base.join("instances").join(instance_id)
    }

    pub fn natives_dir(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id).join("natives")
    }
}
