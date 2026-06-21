//! macOS Apple Silicon 原生啟動支援（Prism Launcher 式函式庫替換）。
//!
//! Mojang 從 1.19 起才提供 `natives-macos-arm64`；更舊的版本：
//! - 1.13–1.18（LWJGL 3.2.x）→ 整組換成 LWJGL 3.3.1（**全部來自 Mojang CDN**，
//!   即 1.19.2 版本 JSON 內的官方 artifact），arm64 Java 原生執行
//! - ≤1.12（LWJGL 2）→ base jar 仍用 Mojang CDN 的 2.9.4，
//!   natives 換成社群 arm64 編譯（與 Prism Launcher meta 相同來源）
//!
//! 替換僅在「實際使用 arm64 Java」時啟用；x86_64 Java（Rosetta）維持原版函式庫。

use crate::mc_parser::version_supports_macos_arm64;
use crate::mc_types::McSpecificVersionDetail;

#[derive(Debug)]
pub struct CompatArtifact {
    /// libraries 目錄下的相對路徑
    pub rel_path: &'static str,
    pub url: &'static str,
    pub sha1: &'static str,
    pub size: u64,
    /// true：natives jar，解壓至 natives_dir（LWJGL2）；
    /// false：直接上 classpath（LWJGL3 natives 由其 loader 自行從 classpath 解壓）
    pub extract: bool,
}

#[derive(Debug)]
pub struct MacosArm64Override {
    pub name: &'static str,
    /// 名稱符合這些前綴的原始 lib 一律跳過（不下載、不上 classpath、不解壓）
    pub exclude_prefixes: &'static [&'static str],
    pub artifacts: &'static [CompatArtifact],
}

impl MacosArm64Override {
    pub fn excludes(&self, lib_name: &str) -> bool {
        self.exclude_prefixes
            .iter()
            .any(|p| lib_name.starts_with(p))
    }
}

/// 若此版本可用函式庫替換在 Apple Silicon 上原生執行，回傳替換表。
/// 呼叫端需自行確認平台為 macOS aarch64 且實際使用 arm64 Java。
pub fn arm64_override_for(
    version: &McSpecificVersionDetail,
) -> Option<&'static MacosArm64Override> {
    if version_supports_macos_arm64(version) {
        return None; // 1.19+ 原生支援，不需替換
    }
    if version
        .libraries
        .iter()
        .any(|l| l.name.starts_with("org.lwjgl:"))
    {
        return Some(&LWJGL3_OVERRIDE); // 1.13–1.18
    }
    if version
        .libraries
        .iter()
        .any(|l| l.name.starts_with("org.lwjgl.lwjgl:"))
    {
        return Some(&LWJGL2_OVERRIDE); // ≤1.12
    }
    None
}

/// 1.13–1.18：LWJGL 3.2.x → 3.3.1（資料取自 Mojang 1.19.2 版本 JSON，全為官方 CDN）
static LWJGL3_OVERRIDE: MacosArm64Override = MacosArm64Override {
    name: "lwjgl3-3.3.1-arm64",
    exclude_prefixes: &["org.lwjgl:", "ca.weblite:java-objc-bridge:"],
    artifacts: &[
        // ── base jars ──
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
            sha1: "ae58664f88e18a9bb2c77b063833ca7aaec484cb",
            size: 724243,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1.jar",
            sha1: "a817bcf213db49f710603677457567c37d53e103",
            size: 36601,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1.jar",
            sha1: "2623a6b8ae1dfcd880738656a9f0243d2e6840bd",
            size: 88237,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1.jar",
            sha1: "831a5533a21a5f4f81bbc51bb13e9899319b5411",
            size: 921563,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1.jar",
            sha1: "cbac1b8d30cb4795149c1ef540f912671a8616d0",
            size: 128801,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1.jar",
            sha1: "b119297cf8ed01f247abe8685857f8e7fcf5980f",
            size: 112380,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1.jar",
            sha1: "0ff1914111ef2e3e0110ef2dabc8d8cdaad82347",
            size: 6767,
            extract: false,
        },
        // ── natives-macos-arm64（上 classpath，LWJGL3 自行解壓）──
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar",
            sha1: "71d0d5e469c9c95351eb949064497e3391616ac9",
            size: 42693,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos-arm64.jar",
            sha1: "e577b87d8ad2ade361aaea2fcf226c660b15dee8",
            size: 103475,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos-arm64.jar",
            sha1: "23d55e7490b57495320f6c9e1936d78fd72c4ef8",
            size: 346125,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos-arm64.jar",
            sha1: "eafe34b871d966292e8db0f1f3d6b8b110d4e91d",
            size: 41665,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar",
            sha1: "cac0d3f712a3da7641fa174735a5f315de7ffe0a",
            size: 129077,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos-arm64.jar",
            sha1: "fcf073ed911752abdca5f0b00a53cfdf17ff8e8b",
            size: 178408,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos-arm64.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos-arm64.jar",
            sha1: "972ecc17bad3571e81162153077b4d47b7b9eaa9",
            size: 41380,
            extract: false,
        },
        // ── java-objc-bridge 1.0.0（僅 x64 dylib）→ 1.1（Mojang CDN，內含 universal dylib）──
        CompatArtifact {
            rel_path: "ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar",
            url: "https://libraries.minecraft.net/ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar",
            sha1: "1227f9e0666314f9de41477e3ec277e542ed7f7b",
            size: 1330045,
            extract: false,
        },
    ],
};

/// ≤1.12：LWJGL 2 base jar 用 Mojang CDN 2.9.4，natives 用社群 arm64 編譯
/// （來源與 Prism Launcher meta 相同：MinecraftMachina / r58Playz）
static LWJGL2_OVERRIDE: MacosArm64Override = MacosArm64Override {
    name: "lwjgl2-2.9.4-arm64",
    exclude_prefixes: &["org.lwjgl.lwjgl:", "net.java.jinput:jinput-platform:"],
    artifacts: &[
        // ── base jars（Mojang CDN）──
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/lwjgl/2.9.4-nightly-20150209/lwjgl-2.9.4-nightly-20150209.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl/2.9.4-nightly-20150209/lwjgl-2.9.4-nightly-20150209.jar",
            sha1: "697517568c68e78ae0b4544145af031c81082dfe",
            size: 1047168,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/lwjgl_util/2.9.4-nightly-20150209/lwjgl_util-2.9.4-nightly-20150209.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl_util/2.9.4-nightly-20150209/lwjgl_util-2.9.4-nightly-20150209.jar",
            sha1: "d51a7c040a721d13efdfbd34f8b257b2df882ad0",
            size: 173887,
            extract: false,
        },
        // ── arm64 natives（社群編譯，解壓至 natives_dir）──
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-osx-arm64.jar",
            url: "https://github.com/MinecraftMachina/lwjgl/releases/download/2.9.4-20150209-mmachina.2/lwjgl-platform-2.9.4-nightly-20150209-natives-osx.jar",
            sha1: "eff546c0b319d6ffc7a835652124c18089c67f36",
            size: 488316,
            extract: true,
        },
        CompatArtifact {
            rel_path: "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-osx-arm64.jar",
            url: "https://github.com/r58Playz/jinput-m1/raw/main/plugins/OSX/bin/jinput-platform-2.0.5.jar",
            sha1: "5189eb40db3087fb11ca063b68fa4f4c20b199dd",
            size: 10031,
            extract: true,
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn load_version(path: &str) -> McSpecificVersionDetail {
        let data = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("找不到 {path}"));
        serde_json::from_str(&data).unwrap_or_else(|e| panic!("解析 {path} 失敗：{e}"))
    }

    #[test]
    fn test_override_selection() {
        // 1.19+：原生支援，不替換
        assert!(arm64_override_for(&load_version("data/1.21.json")).is_none());
        assert!(arm64_override_for(&load_version("data/1.19.2.json")).is_none());
        // 1.13–1.18：LWJGL3 替換
        assert_eq!(
            arm64_override_for(&load_version("data/1.18.1.json")).map(|o| o.name),
            Some("lwjgl3-3.3.1-arm64")
        );
        assert_eq!(
            arm64_override_for(&load_version("data/1.14.4.json")).map(|o| o.name),
            Some("lwjgl3-3.3.1-arm64")
        );
        // ≤1.12：LWJGL2 替換
        assert_eq!(
            arm64_override_for(&load_version("data/1.12.2.json")).map(|o| o.name),
            Some("lwjgl2-2.9.4-arm64")
        );
        assert_eq!(
            arm64_override_for(&load_version("data/1.8.9.json")).map(|o| o.name),
            Some("lwjgl2-2.9.4-arm64")
        );
    }

    #[test]
    fn test_excludes() {
        let ov = arm64_override_for(&load_version("data/1.18.1.json")).unwrap();
        assert!(ov.excludes("org.lwjgl:lwjgl:3.2.1"));
        assert!(ov.excludes("org.lwjgl:lwjgl-glfw:3.2.1"));
        assert!(ov.excludes("ca.weblite:java-objc-bridge:1.0.0"));
        assert!(!ov.excludes("com.mojang:patchy:2.1.6"));
        assert!(!ov.excludes("net.java.dev.jna:jna:5.10.0"));

        let ov2 = arm64_override_for(&load_version("data/1.12.2.json")).unwrap();
        assert!(ov2.excludes("org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"));
        assert!(ov2.excludes("org.lwjgl.lwjgl:lwjgl-platform:2.9.2-nightly-20140822"));
        assert!(ov2.excludes("net.java.jinput:jinput-platform:2.0.5"));
        assert!(!ov2.excludes("net.java.jinput:jinput:2.0.5"));
        assert!(!ov2.excludes("net.java.jutils:jutils:1.0.0"));
    }
}
