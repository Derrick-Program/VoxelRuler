slint::include_modules!();
mod mc_token;
#[tokio::main]
async fn main() -> anyhow::Result<()>{
    let app = MainApp::new()?;
    // slint::select_bundled_translation("zh_TW").unwrap();
    app.run()?;
    Ok(())
}