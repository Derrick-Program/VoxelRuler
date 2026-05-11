use slint_build::EmbedResourcesKind;

fn main() {
    let pj_path = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let pj_path = std::path::Path::new(&pj_path);
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations(pj_path.join("ui/translations"))
        .with_library_paths(std::collections::HashMap::from([
            (
                "material".to_string(),
                pj_path.join("ui/libs/material-1.0/material.slint"),
            ),
            ("theme".to_string(), pj_path.join("ui/theme.slint")),
            ("assets".to_string(), pj_path.join("ui/assets/index.slint")),
            ("global".to_string(), pj_path.join("ui/global.slint")),
        ]))
        .with_include_paths(vec![std::path::PathBuf::from("ui")])
        .embed_resources(EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config(pj_path.join("ui/main.slint"), config).unwrap();
}
