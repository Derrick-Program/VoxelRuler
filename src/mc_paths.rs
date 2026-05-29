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
        let d = self.base.join("versions");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        let d = self.versions_dir().join(version_id);
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn version_jar(&self, version_id: &str) -> PathBuf {
        let d = self
            .version_dir(version_id)
            .join(format!("{}.jar", version_id));
        if !d.exists() {
            std::fs::File::create(&d).ok();
        }
        d
    }

    pub fn libraries_dir(&self) -> PathBuf {
        let d = self.base.join("libraries");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn assets_dir(&self) -> PathBuf {
        let d = self.base.join("assets");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn asset_indexes_dir(&self) -> PathBuf {
        let d = self.assets_dir().join("indexes");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        let d = self.assets_dir().join("objects");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn java_dir(&self, component: &str) -> PathBuf {
        let d = self.base.join("java").join(component);
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn java_bin(&self, component: &str) -> PathBuf {
        #[cfg(target_os = "windows")]
        return self.java_dir(component).join("bin").join("javaw.exe");
        #[cfg(not(target_os = "windows"))]
        return self
            .java_dir(component)
            .join("jre.bundle")
            .join("Contents")
            .join("Home")
            .join("bin")
            .join("java");
    }

    pub fn instances_base_dir(&self) -> PathBuf {
        let d = self.base.join("instances");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        let d = self.base.join("instances").join(instance_id);
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    pub fn natives_dir(&self, version_id: &str) -> PathBuf {
        let d = self.version_dir(version_id).join("natives");
        if !d.exists() {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }
}
