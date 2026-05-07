slint::include_modules!();
fn main() -> anyhow::Result<()>{
    // slint::select_bundled_translation("").unwrap();
    // slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/translations"));
    let app = MainApp::new()?;
    // let app_weak = app.as_weak();
    app.run()?;
    Ok(())
}