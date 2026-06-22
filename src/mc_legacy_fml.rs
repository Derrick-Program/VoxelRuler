use std::path::Path;

const PRISM_FMLLIBS_BASE: &str = "https://files.prismlauncher.org/fmllibs/";

/// Returns the list of FML lib filenames required for the given Forge version_id.
///
/// version_id is the Forge profile ID, e.g. "1.5.2-Forge9.11.1.965".
/// The MC version is extracted by splitting on '-' and taking the first segment.
pub fn get_fmllib_filenames(version_id: &str) -> &'static [&'static str] {
    let mc_ver = version_id.split('-').next().unwrap_or(version_id);

    match mc_ver {
        v if v.starts_with("1.4.") => &[
            "bcprov-jdk15on-147.jar",
            "argo-2.25.jar",
            "guava-12.0.1.jar",
            "asm-all-4.0.jar",
        ],
        "1.5" => &[
            "deobfuscation_data_1.5.zip",
            "bcprov-jdk15on-147.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
        ],
        "1.5.1" => &[
            "deobfuscation_data_1.5.1.zip",
            "bcprov-jdk15on-147.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
        ],
        "1.5.2" => &[
            "deobfuscation_data_1.5.2.zip",
            "bcprov-jdk15on-147.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
        ],
        "1.6.1" => &[
            "deobfuscation_data_1.6.1.zip",
            "bcprov-jdk15on-148.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
            "lzma-0.0.1.jar",
        ],
        "1.6.2" => &[
            "deobfuscation_data_1.6.2.zip",
            "bcprov-jdk15on-148.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
            "lzma-0.0.1.jar",
        ],
        "1.6.3" => &[
            "deobfuscation_data_1.6.3.zip",
            "bcprov-jdk15on-148.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
            "lzma-0.0.1.jar",
        ],
        "1.6.4" => &[
            "deobfuscation_data_1.6.4.zip",
            "bcprov-jdk15on-148.jar",
            "argo-small-3.2.jar",
            "guava-14.0-rc3.jar",
            "asm-all-4.1.jar",
            "lzma-0.0.1.jar",
        ],
        _ => &[],
    }
}

/// Downloads all required FML libs for the given Forge version into `libraries_dir/fmllibs/`.
/// Uses best-effort download (no SHA1 verification); skips files that already exist.
pub async fn install_fmllibs(version_id: &str, libraries_dir: &Path) -> anyhow::Result<()> {
    let filenames = get_fmllib_filenames(version_id);
    if filenames.is_empty() {
        return Ok(());
    }
    let fmllib_dir = libraries_dir.join("fmllibs");
    tokio::fs::create_dir_all(&fmllib_dir).await?;
    for filename in filenames {
        let url = format!("{}{}", PRISM_FMLLIBS_BASE, filename);
        let dest = fmllib_dir.join(filename);
        tracing::info!(filename, "下載舊版 FML 依賴");
        crate::mc_install::download_best_effort(&url, &dest).await?;
    }
    Ok(())
}

/// Copies downloaded FML libs from `libraries_dir/fmllibs/` into `game_dir/lib/`
/// so that Forge's FML bootstrap can find them at launch time.
pub async fn copy_fmllibs_to_game_dir(
    version_id: &str,
    libraries_dir: &Path,
    game_dir: &Path,
) -> anyhow::Result<()> {
    let filenames = get_fmllib_filenames(version_id);
    if filenames.is_empty() {
        return Ok(());
    }
    let lib_dir = game_dir.join("lib");
    tokio::fs::create_dir_all(&lib_dir).await?;
    for filename in filenames {
        let source = libraries_dir.join("fmllibs").join(filename);
        let dest = lib_dir.join(filename);
        if source.exists() {
            tracing::info!(filename, "複製舊版 FML 依賴至實例目錄");
            tokio::fs::copy(&source, &dest).await?;
        }
    }
    Ok(())
}
