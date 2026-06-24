use crate::view::SkinData;
use crate::view::*;
use slint::Image;
use slint::SharedString;

pub(crate) fn create_cape_preview_raw(img: &image::DynamicImage) -> (Vec<u8>, u32, u32) {
    use image::GenericImageView;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 64 && h == 32 {
        let mut preview = image::RgbaImage::new(22, 16);
        for y in 0..16 {
            for x in 0..10 {
                preview.put_pixel(x, y, *rgba.get_pixel(1 + x, 1 + y));
            }
        }
        for y in 0..16 {
            for x in 0..10 {
                preview.put_pixel(12 + x, y, *rgba.get_pixel(12 + x, 1 + y));
            }
        }
        (preview.into_raw(), 22, 16)
    } else {
        (rgba.into_raw(), w, h)
    }
}

pub(crate) fn generate_2d_front(
    skin_img: &image::DynamicImage,
    is_slim: bool,
) -> image::DynamicImage {
    use image::{GenericImage, imageops};

    let mut out = image::DynamicImage::new_rgba8(16, 32);
    let is_64x64 = skin_img.height() == 64;

    let mut head = skin_img.crop_imm(8, 8, 8, 8);
    let hat = skin_img.crop_imm(40, 8, 8, 8);
    imageops::overlay(&mut head, &hat, 0, 0);
    imageops::overlay(&mut out, &head, 4, 0);

    let mut body = skin_img.crop_imm(20, 20, 8, 12);
    if is_64x64 {
        let jacket = skin_img.crop_imm(20, 36, 8, 12);
        imageops::overlay(&mut body, &jacket, 0, 0);
    }
    imageops::overlay(&mut out, &body, 4, 8);

    let arm_w = if is_slim { 3 } else { 4 };

    let mut r_arm = skin_img.crop_imm(44, 20, arm_w, 12);
    if is_64x64 {
        let r_sleeve = skin_img.crop_imm(44, 36, arm_w, 12);
        imageops::overlay(&mut r_arm, &r_sleeve, 0, 0);
    }
    let r_arm_x = if is_slim { 1 } else { 0 };
    imageops::overlay(&mut out, &r_arm, r_arm_x, 8);

    let mut r_leg = skin_img.crop_imm(4, 20, 4, 12);
    if is_64x64 {
        let r_pants = skin_img.crop_imm(4, 36, 4, 12);
        imageops::overlay(&mut r_leg, &r_pants, 0, 0);
    }
    imageops::overlay(&mut out, &r_leg, 4, 20);

    let mut l_arm = if is_64x64 {
        skin_img.crop_imm(36, 52, arm_w, 12)
    } else {
        let mut arm = skin_img.crop_imm(44, 20, arm_w, 12);
        imageops::flip_horizontal_in_place(&mut arm);
        arm
    };
    if is_64x64 {
        let l_sleeve = skin_img.crop_imm(52, 52, arm_w, 12);
        imageops::overlay(&mut l_arm, &l_sleeve, 0, 0);
    }
    imageops::overlay(&mut out, &l_arm, 12, 8);

    let mut l_leg = if is_64x64 {
        skin_img.crop_imm(20, 52, 4, 12)
    } else {
        let mut leg = skin_img.crop_imm(4, 20, 4, 12);
        imageops::flip_horizontal_in_place(&mut leg);
        leg
    };
    if is_64x64 {
        let l_pants = skin_img.crop_imm(4, 52, 4, 12);
        imageops::overlay(&mut l_leg, &l_pants, 0, 0);
    }
    imageops::overlay(&mut out, &l_leg, 8, 20);

    out.resize(16 * 10, 32 * 10, image::imageops::FilterType::Nearest)
}

pub(crate) fn detect_is_slim(img: &image::DynamicImage) -> bool {
    if img.height() == 32 {
        return false;
    }
    use image::GenericImageView;
    if img.width() >= 64 && img.height() >= 64 {
        let pixel = img.get_pixel(54, 20);
        pixel.0[3] == 0
    } else {
        false
    }
}

pub(crate) fn get_ui_skins(
    paths: &crate::mc_paths::McPaths,
    history: &crate::skin_history::SkinHistory,
) -> Vec<SkinData> {
    let mut ui_skins = Vec::new();
    for skin in &history.skins {
        let hash = skin
            .url
            .split('/')
            .last()
            .unwrap_or(&skin.name)
            .trim_end_matches(".png");
        let render_path = paths.skins_dir().join(format!("{}_render.png", hash));
        let skin_path = paths.skins_dir().join(format!("{}.png", hash));

        if !render_path.exists() && skin_path.exists() {
            if let Ok(img) = image::open(&skin_path) {
                let is_slim = skin.model == "slim";
                let render_img = generate_2d_front(&img, is_slim);
                let _ = render_img.save(&render_path);
            }
        }

        let has_preview = render_path.exists() || skin_path.exists();
        let preview_image = if render_path.exists() {
            slint::Image::load_from_path(&render_path).unwrap_or_default()
        } else if skin_path.exists() {
            slint::Image::load_from_path(&skin_path).unwrap_or_default()
        } else {
            slint::Image::default()
        };

        ui_skins.push(SkinData {
            id: skin.name.clone().into(),
            name: skin.name.clone().into(),
            variant: skin.model.clone().into(),
            url: skin.url.clone().into(),
            preview_image,
            has_preview,
        });
    }
    ui_skins
}

pub(crate) async fn fetch_avatar_from_mojang(
    token: &str,
    username: &str,
    add_to_library: bool,
) -> Option<(std::path::PathBuf, String)> {
    let api = crate::mc_api::McAction::new().authenticate(token);
    let profile = api.get_user_profile().await.ok()?;
    let active_skin = profile
        .skins
        .iter()
        .find(|s| s.state == crate::mc_types::McState::Active)?;

    let skin_bytes = reqwest::get(&active_skin.url)
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;

    let variant = if active_skin.variant == crate::mc_types::McSkinVariant::Slim {
        "slim".to_string()
    } else {
        "classic".to_string()
    };

    // Save to local skins folder and update history
    if let Ok(paths) = crate::mc_paths::McPaths::new() {
        let history_file = paths.skins_history_file();
        let mut history = crate::skin_history::SkinHistory::load(&history_file);

        use sha1::Digest;
        let mut pixel_hash = String::new();
        if let Ok(img) = image::load_from_memory(&skin_bytes) {
            let mut hasher = sha1::Sha1::new();
            hasher.update(img.to_rgba8().into_raw());
            pixel_hash = hasher
                .finalize()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
        } else {
            let mut hasher = sha1::Sha1::new();
            hasher.update(&skin_bytes);
            pixel_hash = hasher
                .finalize()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
        }

        let url = active_skin.url.clone();
        let mojang_hash = url.split('/').last().unwrap_or(&active_skin.id).to_string();

        let skin_path = paths.skins_dir().join(format!("{}.png", mojang_hash));
        let render_path = paths
            .skins_dir()
            .join(format!("{}_render.png", mojang_hash));
        let _ = std::fs::write(&skin_path, &skin_bytes);

        if let Ok(img) = image::load_from_memory(&skin_bytes) {
            let is_slim = variant == "slim";
            let render_img = generate_2d_front(&img, is_slim);
            let _ = render_img.save(&render_path);
        }

        if add_to_library {
            // Avoid adding duplicate if it already exists (check SHA-1 of pixels or exact URL)
            let already_exists = history.skins.iter().any(|s| {
                s.url == url
                    || s.url.ends_with(&format!("{}.png", pixel_hash))
                    || s.url
                        .split('/')
                        .last()
                        .unwrap_or("")
                        .trim_end_matches(".png")
                        == pixel_hash
            });

            if !already_exists {
                history.add_skin(crate::skin_history::SkinEntry {
                    cape_id: "".to_string(),
                    model: variant,
                    name: username.to_string(),
                    url, // Store the Mojang URL
                });
                let _ = history.save(&history_file);
            }
        }
    }

    let img = image::load_from_memory(&skin_bytes).ok()?;
    let mut face = img.crop_imm(8, 8, 8, 8);
    let overlay = img.crop_imm(40, 8, 8, 8);
    image::imageops::overlay(&mut face, &overlay, 0, 0);

    let scaled = image::imageops::resize(&face, 100, 100, image::imageops::FilterType::Nearest);
    let cache_dir = std::env::temp_dir().join("voxelruler_avatars");
    std::fs::create_dir_all(&cache_dir).ok()?;

    let path = cache_dir.join(format!("{}_mojang.png", username));
    scaled.save(&path).ok()?;
    Some((path, active_skin.url.clone()))
}

pub(crate) async fn fetch_avatar_path(username: &str) -> Option<std::path::PathBuf> {
    let cache_dir = std::env::temp_dir().join("voxelruler_avatars");
    let _ = std::fs::create_dir_all(&cache_dir);
    let avatar_path = cache_dir.join(format!("{}.png", username));

    if avatar_path.exists() {
        return Some(avatar_path);
    }

    let url = format!("https://minotar.net/helm/{}/100.png", username);
    if let Ok(resp) = reqwest::get(url).await
        && let Ok(bytes) = resp.bytes().await
        && std::fs::write(&avatar_path, bytes).is_ok()
    {
        return Some(avatar_path);
    }
    None
}
