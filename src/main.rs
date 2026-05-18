use crate::view::open_view;

mod mc_token;
mod mc_action;
mod mc_types;
mod view;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pdir = directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler").unwrap();
    dbg!(pdir);
    open_view().await?;
    // mc_token::get_minecraft_token().await?;
    Ok(())
}
