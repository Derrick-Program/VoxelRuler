slint::include_modules!();
fn main() -> anyhow::Result<()>{
    // slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/translations"));
    let app = MainApp::new()?;
    slint::select_bundled_translation("zh_TW").unwrap();
    // let app_weak = app.as_weak();
    app.run()?;
    Ok(())
}