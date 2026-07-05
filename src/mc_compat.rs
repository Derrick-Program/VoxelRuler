use crate::mc_parser::version_supports_macos_arm64;
use crate::mc_types::McSpecificVersionDetail;

#[derive(Debug)]
pub struct CompatArtifact {
    pub rel_path: &'static str,
    pub url: &'static str,
    pub sha1: &'static str,
    pub size: u64,
    pub extract: bool,
}

#[derive(Debug)]
pub struct MacosArm64Override {
    pub name: &'static str,
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

pub fn arm64_override_for(
    version: &McSpecificVersionDetail,
) -> Option<&'static MacosArm64Override> {
    if version_supports_macos_arm64(version) {
        return None;
    }
    if version
        .libraries
        .iter()
        .any(|l| l.name.starts_with("org.lwjgl:"))
    {
        return Some(&LWJGL3_OVERRIDE);
    }
    if version
        .libraries
        .iter()
        .any(|l| l.name.starts_with("org.lwjgl.lwjgl:"))
    {
        return Some(&LWJGL2_OVERRIDE);
    }
    None
}

pub fn macos_override_for(
    version: &McSpecificVersionDetail,
    arm64_java: bool,
) -> Option<&'static MacosArm64Override> {
    if arm64_java {
        return arm64_override_for(version);
    }
    if version_supports_macos_arm64(version) {
        return None;
    }
    version
        .libraries
        .iter()
        .any(|l| l.name.starts_with("org.lwjgl:"))
        .then_some(&LWJGL3_X64_OVERRIDE)
}

static LWJGL3_OVERRIDE: MacosArm64Override = MacosArm64Override {
    name: "lwjgl3-3.3.1-arm64",
    exclude_prefixes: &["org.lwjgl:", "ca.weblite:java-objc-bridge:"],
    artifacts: &[
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
        MMACHINA_GLFW_BINDINGS,
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
        CompatArtifact {
            rel_path: "ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar",
            url: "https://libraries.minecraft.net/ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar",
            sha1: "1227f9e0666314f9de41477e3ec277e542ed7f7b",
            size: 1330045,
            extract: false,
        },
    ],
};

const MMACHINA_GLFW_BINDINGS: CompatArtifact = CompatArtifact {
    rel_path: "org/lwjgl/lwjgl-glfw/3.3.1-mmachina.1/lwjgl-glfw-3.3.1-mmachina.1.jar",
    url: "https://github.com/MinecraftMachina/lwjgl3/releases/download/3.3.1-mmachina.1/lwjgl-glfw.jar",
    sha1: "e9a101bca4fa30d26b21b526ff28e7c2d8927f1b",
    size: 130128,
    extract: false,
};

static LWJGL3_X64_OVERRIDE: MacosArm64Override = MacosArm64Override {
    name: "lwjgl3-3.3.1-x64",
    exclude_prefixes: &["org.lwjgl:"],
    artifacts: &[
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
        MMACHINA_GLFW_BINDINGS,
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
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos.jar",
            sha1: "fc6bb723dec2cd031557dccb2a95f0ab80acb9db",
            size: 55706,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos.jar",
            sha1: "56424dc8db3cfb8e7b594aa6d59a4f4387b7f544",
            size: 117480,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos.jar",
            sha1: "3a57b8911835fb58b5e558d0ca0d28157e263d45",
            size: 397196,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos.jar",
            sha1: "a0d12697ea019a7362eff26475b0531340e876a6",
            size: 40709,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos.jar",
            sha1: "9ec4ce1fc8c85fdef03ef4ff2aace6f5775fb280",
            size: 131655,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos.jar",
            sha1: "def8879b8d38a47a4cc1d48b1f9a7b772e51258e",
            size: 203582,
            extract: false,
        },
        CompatArtifact {
            rel_path: "org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos.jar",
            url: "https://libraries.minecraft.net/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos.jar",
            sha1: "78641a0fa5e9afa714adfdd152c357930c97a1ce",
            size: 44821,
            extract: false,
        },
    ],
};

static LWJGL2_OVERRIDE: MacosArm64Override = MacosArm64Override {
    name: "lwjgl2-2.9.4-arm64",
    exclude_prefixes: &["org.lwjgl.lwjgl:", "net.java.jinput:jinput-platform:"],
    artifacts: &[
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

struct CrashSignature {
    os: Option<&'static str>,
    needles: &'static [&'static str],
    advice: &'static str,
}

const ADVICE_NO_HW_ACCEL: &str = "the GPU driver has no hardware-accelerated OpenGL \
    (common on older Intel HD graphics with outdated Windows drivers). \
    Update the GPU driver from the vendor website, then retry.";

const ADVICE_GL_CORE: &str = "this Minecraft version requires OpenGL 3.2+ Core Profile \
    (mandatory since 1.17) and the GPU/driver only exposes an older OpenGL version. \
    Update the GPU driver, or use Minecraft 1.16.5 or older on this hardware.";

const ADVICE_FIRST_THREAD: &str = "macOS requires the JVM flag -XstartOnFirstThread for \
    this version (LWJGL 3 / GLFW). It was removed by custom JVM arguments or a mod \
    loader profile — check the instance's Java settings.";

const ADVICE_VULKAN: &str = "the experimental Vulkan renderer (26.2+) failed to \
    initialize — the GPU/driver lacks Vulkan 1.2 support (on macOS this goes through \
    MoltenVK). Switch the renderer back to OpenGL in Video Settings, or update the \
    GPU driver.";

const ADVICE_DISPLAY: &str = "no usable display connection (Linux). On Wayland, make \
    sure XWayland is available (DISPLAY is set for this app); versions up to 1.12.2 \
    (LWJGL 2) require X11/XWayland.";

const ADVICE_NATIVES: &str = "a native library failed to load — usually a missing file \
    or a CPU architecture mismatch (e.g. x86_64 natives with an arm64 Java). Retry to \
    re-download the libraries; on Apple Silicon, versions before 1.19 use replaced \
    arm64 libraries or x86_64 Java via Rosetta.";

const ADVICE_LWJGL2_MACOS: &str = "the JVM crashed inside LWJGL 2 native code — a known \
    macOS issue with versions up to 1.12.2 (mouse/pointer handling on newer macOS). \
    On Apple Silicon use the arm64 native mode (patched LWJGL 2); otherwise use \
    x86_64 Java via Rosetta.";

const ADVICE_MACOS_OLD_GLFW: &str = "the GLFW bundled with this Minecraft version \
    (1.13–1.16) is too old for this macOS release ('failed to find service port for \
    display'). The launcher normally substitutes updated LWJGL automatically — \
    try launching again; if it persists, check the instance log for library errors.";

const ADVICE_MACOS_GLFW_TOO_NEW: &str = "this Minecraft version (1.13-1.18) sets a window \
    icon during startup, which LWJGL 3.3+ (GLFW 3.4) rejects on macOS and the game \
    treats as fatal. Set this instance's Java to 'Follow Minecraft' (the launcher then \
    picks Rosetta + LWJGL 3.2.3, which avoids this) instead of a custom arm64 Java.";

static CRASH_SIGNATURES: &[CrashSignature] = &[
    CrashSignature {
        os: Some("windows"),
        needles: &["Pixel format not accelerated"],
        advice: ADVICE_NO_HW_ACCEL,
    },
    CrashSignature {
        os: Some("windows"),
        needles: &["WGL: The driver does not appear to support OpenGL"],
        advice: ADVICE_NO_HW_ACCEL,
    },
    CrashSignature {
        os: None,
        needles: &["GLFW error 65543"],
        advice: ADVICE_GL_CORE,
    },
    CrashSignature {
        os: Some("linux"),
        needles: &["GLX: Failed to create context"],
        advice: ADVICE_GL_CORE,
    },
    CrashSignature {
        os: Some("macos"),
        needles: &["-XstartOnFirstThread"],
        advice: ADVICE_FIRST_THREAD,
    },
    CrashSignature {
        os: None,
        needles: &["VK_ERROR_INCOMPATIBLE_DRIVER"],
        advice: ADVICE_VULKAN,
    },
    CrashSignature {
        os: None,
        needles: &["VK_ERROR_INITIALIZATION_FAILED"],
        advice: ADVICE_VULKAN,
    },
    CrashSignature {
        os: None,
        needles: &["Failed to initialize Vulkan"],
        advice: ADVICE_VULKAN,
    },
    CrashSignature {
        os: Some("macos"),
        needles: &["GLFW error 65544", "service port for display"],
        advice: ADVICE_MACOS_OLD_GLFW,
    },
    CrashSignature {
        os: Some("macos"),
        needles: &["GLFW error 65548"],
        advice: ADVICE_MACOS_GLFW_TOO_NEW,
    },
    CrashSignature {
        os: Some("linux"),
        needles: &["GLFW error 65544"],
        advice: ADVICE_DISPLAY,
    },
    CrashSignature {
        os: Some("linux"),
        needles: &["X11: Failed to open display"],
        advice: ADVICE_DISPLAY,
    },
    CrashSignature {
        os: Some("linux"),
        needles: &["Failed to connect to the Wayland display"],
        advice: ADVICE_DISPLAY,
    },
    CrashSignature {
        os: Some("macos"),
        needles: &["A fatal error has been detected", "liblwjgl"],
        advice: ADVICE_LWJGL2_MACOS,
    },
    CrashSignature {
        os: None,
        needles: &["java.lang.UnsatisfiedLinkError"],
        advice: ADVICE_NATIVES,
    },
    CrashSignature {
        os: None,
        needles: &["Failed to locate library"],
        advice: ADVICE_NATIVES,
    },
];

pub fn diagnose_graphics_crash<'a, I>(lines: I) -> Option<&'static str>
where
    I: IntoIterator<Item = &'a str>,
{
    diagnose_with_os(lines, std::env::consts::OS)
}

fn diagnose_with_os<'a, I>(lines: I, os: &str) -> Option<&'static str>
where
    I: IntoIterator<Item = &'a str>,
{
    let joined = lines
        .into_iter()
        .map(|l| l.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    CRASH_SIGNATURES
        .iter()
        .filter(|sig| sig.os.is_none_or(|o| o == os))
        .find(|sig| {
            sig.needles
                .iter()
                .all(|n| joined.contains(&n.to_ascii_lowercase()))
        })
        .map(|sig| sig.advice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_version(path: &str) -> McSpecificVersionDetail {
        let data =
            std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Could not find {path}"));
        serde_json::from_str(&data).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
    }

    #[test]
    fn test_override_selection() {
        assert!(arm64_override_for(&load_version("data/1.21.json")).is_none());
        assert!(arm64_override_for(&load_version("data/1.19.2.json")).is_none());
        assert_eq!(
            arm64_override_for(&load_version("data/1.18.1.json")).map(|o| o.name),
            Some("lwjgl3-3.3.1-arm64")
        );
        assert_eq!(
            arm64_override_for(&load_version("data/1.14.4.json")).map(|o| o.name),
            Some("lwjgl3-3.3.1-arm64")
        );
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

    #[test]
    fn test_macos_override_by_java_arch() {
        assert!(macos_override_for(&load_version("data/1.19.2.json"), true).is_none());
        assert!(macos_override_for(&load_version("data/1.19.2.json"), false).is_none());
        assert_eq!(
            macos_override_for(&load_version("data/1.14.4.json"), true).map(|o| o.name),
            Some("lwjgl3-3.3.1-arm64")
        );
        assert_eq!(
            macos_override_for(&load_version("data/1.14.4.json"), false).map(|o| o.name),
            Some("lwjgl3-3.3.1-x64")
        );
        assert_eq!(
            macos_override_for(&load_version("data/1.12.2.json"), true).map(|o| o.name),
            Some("lwjgl2-2.9.4-arm64")
        );
        assert!(macos_override_for(&load_version("data/1.12.2.json"), false).is_none());

        let ov = macos_override_for(&load_version("data/1.14.4.json"), false).unwrap();
        assert!(ov.excludes("org.lwjgl:lwjgl:3.2.1"));
        assert!(ov.excludes("org.lwjgl:lwjgl-glfw:3.2.1"));
        assert!(!ov.excludes("ca.weblite:java-objc-bridge:1.0.0"));
        assert!(!ov.excludes("com.mojang:patchy:1.1"));
    }

    #[test]
    fn test_diagnose_known_signatures() {
        let log = ["org.lwjgl.LWJGLException: PIXEL FORMAT NOT ACCELERATED"];
        assert_eq!(diagnose_with_os(log, "windows"), Some(ADVICE_NO_HW_ACCEL));

        let log = [
            "GLFW error 65543: WGL: OpenGL profile requested but WGL_ARB_create_context_profile is unavailable",
        ];
        assert_eq!(diagnose_with_os(log, "windows"), Some(ADVICE_GL_CORE));

        let log = ["[Render thread/ERROR]: vkCreateInstance: VK_ERROR_INCOMPATIBLE_DRIVER"];
        assert_eq!(diagnose_with_os(log, "macos"), Some(ADVICE_VULKAN));

        let log = [
            "# A fatal error has been detected by the Java Runtime Environment:",
            "# Problematic frame:",
            "# C  [liblwjgl.dylib+0x1234]",
        ];
        assert_eq!(diagnose_with_os(log, "macos"), Some(ADVICE_LWJGL2_MACOS));

        let log = ["# A fatal error has been detected by the Java Runtime Environment:"];
        assert_eq!(diagnose_with_os(log, "macos"), None);
    }

    #[test]
    fn test_diagnose_os_gating() {
        let log = ["GLFW error 65544: Cocoa: Failed to find service port for display"];
        assert_eq!(diagnose_with_os(log, "macos"), Some(ADVICE_MACOS_OLD_GLFW));
        assert_eq!(diagnose_with_os(log, "linux"), Some(ADVICE_DISPLAY));

        let log = ["GLFW error 65544: Cocoa: some other platform error"];
        assert_eq!(diagnose_with_os(log, "macos"), None);

        let log = [
            "java.lang.IllegalStateException: GLFW error 65548: Cocoa: Regular windows do not have icons on macOS",
        ];
        assert_eq!(
            diagnose_with_os(log, "macos"),
            Some(ADVICE_MACOS_GLFW_TOO_NEW)
        );

        let log = ["org.lwjgl.LWJGLException: Pixel format not accelerated"];
        assert_eq!(diagnose_with_os(log, "macos"), None);
    }

    #[test]
    fn test_diagnose_priority_and_none() {
        let log = [
            "java.lang.UnsatisfiedLinkError: no lwjgl in java.library.path",
            "org.lwjgl.LWJGLException: Pixel format not accelerated",
        ];
        assert_eq!(diagnose_with_os(log, "windows"), Some(ADVICE_NO_HW_ACCEL));

        let log = ["java.lang.UnsatisfiedLinkError: no lwjgl in java.library.path"];
        assert_eq!(diagnose_with_os(log, "windows"), Some(ADVICE_NATIVES));

        let log = ["[main/INFO]: Stopping!", "SoundEngine shut down"];
        assert_eq!(diagnose_with_os(log, "linux"), None);
    }
}
