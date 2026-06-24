use image::{DynamicImage, GenericImageView, Rgba};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::f32::consts::PI;

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn dot(&self, other: &Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    fn cross(&self, other: &Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
    fn sub(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
    fn normalize(&self) -> Vec3 {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if len == 0.0 {
            *self
        } else {
            Vec3::new(self.x / len, self.y / len, self.z / len)
        }
    }
}

#[derive(Clone, Copy)]
struct Vertex {
    pos: Vec3,
    u: f32,
    v: f32,
    tex: u8,
}

struct Triangle {
    v0: Vertex,
    v1: Vertex,
    v2: Vertex,
}

pub struct SkinRenderer {
    image: image::RgbaImage,
    cape: Option<image::RgbaImage>,
}

impl SkinRenderer {
    pub fn new(img: DynamicImage) -> Self {
        Self {
            image: img.into_rgba8(),
            cape: None,
        }
    }

    pub fn set_cape(&mut self, img: Option<DynamicImage>) {
        self.cape = img.map(|i| i.into_rgba8());
    }

    pub fn get_cape(&self) -> Option<image::RgbaImage> {
        self.cape.clone()
    }

    pub fn set_cape_rgba(&mut self, cape: Option<image::RgbaImage>) {
        self.cape = cape;
    }

    fn add_box(
        &self,
        tris: &mut Vec<Triangle>,
        tex: u8,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        u: f32,
        v: f32,
        tex_w: f32,
        tex_h: f32,
        inflate: f32,
        local_tx: impl Fn(Vec3) -> Vec3,
    ) {
        let p0 = local_tx(Vec3::new(x - inflate, y - inflate, z - inflate));
        let p1 = local_tx(Vec3::new(x + w + inflate, y - inflate, z - inflate));
        let p2 = local_tx(Vec3::new(x + w + inflate, y + h + inflate, z - inflate));
        let p3 = local_tx(Vec3::new(x - inflate, y + h + inflate, z - inflate));
        let p4 = local_tx(Vec3::new(x - inflate, y - inflate, z + d + inflate));
        let p5 = local_tx(Vec3::new(x + w + inflate, y - inflate, z + d + inflate));
        let p6 = local_tx(Vec3::new(x + w + inflate, y + h + inflate, z + d + inflate));
        let p7 = local_tx(Vec3::new(x - inflate, y + h + inflate, z + d + inflate));

        let mut add_face =
            |tl: Vec3, tr: Vec3, br: Vec3, bl: Vec3, tu: f32, tv: f32, tw: f32, th: f32| {
                let t_tl = (tu / tex_w, tv / tex_h);
                let t_tr = ((tu + tw) / tex_w, tv / tex_h);
                let t_br = ((tu + tw) / tex_w, (tv + th) / tex_h);
                let t_bl = (tu / tex_w, (tv + th) / tex_h);

                tris.push(Triangle {
                    v0: Vertex {
                        pos: tl,
                        u: t_tl.0,
                        v: t_tl.1,
                        tex,
                    },
                    v1: Vertex {
                        pos: bl,
                        u: t_bl.0,
                        v: t_bl.1,
                        tex,
                    },
                    v2: Vertex {
                        pos: br,
                        u: t_br.0,
                        v: t_br.1,
                        tex,
                    },
                });
                tris.push(Triangle {
                    v0: Vertex {
                        pos: tl,
                        u: t_tl.0,
                        v: t_tl.1,
                        tex,
                    },
                    v1: Vertex {
                        pos: br,
                        u: t_br.0,
                        v: t_br.1,
                        tex,
                    },
                    v2: Vertex {
                        pos: tr,
                        u: t_tr.0,
                        v: t_tr.1,
                        tex,
                    },
                });
            };

        add_face(p0, p1, p2, p3, u + d, v + d, w, h);

        add_face(p5, p4, p7, p6, u + d + w + d, v + d, w, h);

        add_face(p4, p5, p1, p0, u + d, v, w, d);

        add_face(p3, p2, p6, p7, u + d + w, v, w, d);

        add_face(p4, p0, p3, p7, u, v + d, d, h);

        add_face(p1, p5, p6, p2, u + d + w, v + d, d, h);
    }

    fn add_box_cape(
        &self,
        tris: &mut Vec<Triangle>,
        tex: u8,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        u: f32,
        v: f32,
        tex_w: f32,
        tex_h: f32,
        inflate: f32,
        local_tx: impl Fn(Vec3) -> Vec3,
    ) {
        let p0 = local_tx(Vec3::new(x - inflate, y - inflate, z - inflate));
        let p1 = local_tx(Vec3::new(x + w + inflate, y - inflate, z - inflate));
        let p2 = local_tx(Vec3::new(x + w + inflate, y + h + inflate, z - inflate));
        let p3 = local_tx(Vec3::new(x - inflate, y + h + inflate, z - inflate));
        let p4 = local_tx(Vec3::new(x - inflate, y - inflate, z + d + inflate));
        let p5 = local_tx(Vec3::new(x + w + inflate, y - inflate, z + d + inflate));
        let p6 = local_tx(Vec3::new(x + w + inflate, y + h + inflate, z + d + inflate));
        let p7 = local_tx(Vec3::new(x - inflate, y + h + inflate, z + d + inflate));

        let mut add_face =
            |tl: Vec3, tr: Vec3, br: Vec3, bl: Vec3, tu: f32, tv: f32, tw: f32, th: f32| {
                let t_tl = (tu / tex_w, tv / tex_h);
                let t_tr = ((tu + tw) / tex_w, tv / tex_h);
                let t_br = ((tu + tw) / tex_w, (tv + th) / tex_h);
                let t_bl = (tu / tex_w, (tv + th) / tex_h);

                tris.push(Triangle {
                    v0: Vertex {
                        pos: tl,
                        u: t_tl.0,
                        v: t_tl.1,
                        tex,
                    },
                    v1: Vertex {
                        pos: bl,
                        u: t_bl.0,
                        v: t_bl.1,
                        tex,
                    },
                    v2: Vertex {
                        pos: br,
                        u: t_br.0,
                        v: t_br.1,
                        tex,
                    },
                });
                tris.push(Triangle {
                    v0: Vertex {
                        pos: tl,
                        u: t_tl.0,
                        v: t_tl.1,
                        tex,
                    },
                    v1: Vertex {
                        pos: br,
                        u: t_br.0,
                        v: t_br.1,
                        tex,
                    },
                    v2: Vertex {
                        pos: tr,
                        u: t_tr.0,
                        v: t_tr.1,
                        tex,
                    },
                });
            };

        // For cape, the outward face (Back face of the box, p5, p4, p7, p6) should get u=1 (which is u+d in standard)
        // And the inward face (Front face of the box, p0, p1, p2, p3) should get u=12 (which is u+d+w+d in standard)

        // Front face (inward, touching body)
        add_face(p0, p1, p2, p3, u + d + w + d, v + d, w, h);

        // Back face (outward, with logo)
        // BUT WAIT! If we look at the back face (p5, p4, p7, p6) from the outside, p5 is on the left, p4 is on the right.
        // If we map it with u+d, p5 gets u+d, p4 gets u+d+w.
        // This is correct.
        add_face(p5, p4, p7, p6, u + d, v + d, w, h);

        // Top face
        add_face(p4, p5, p1, p0, u + d, v, w, d);

        // Bottom face
        add_face(p3, p2, p6, p7, u + d + w, v, w, d);

        // Right face
        add_face(p4, p0, p3, p7, u, v + d, d, h);

        // Left face
        add_face(p1, p5, p6, p2, u + d + w, v + d, d, h);
    }

    pub fn render(
        &self,
        width: u32,
        height: u32,
        yaw: f32,
        pitch: f32,
        slim: bool,
        walk_anim: f32,
    ) -> SharedPixelBuffer<Rgba8Pixel> {
        let mut tris = Vec::new();

        let identity = |v: Vec3| v;

        let arm_angle = (walk_anim * std::f32::consts::PI * 2.0).sin() * 0.5;
        let leg_angle = (walk_anim * std::f32::consts::PI * 2.0).sin() * 0.5;

        let arm_w = if slim { 3.0 } else { 4.0 };

        self.add_box(
            &mut tris, 0, -4.0, -12.0, -4.0, 8.0, 8.0, 8.0, 0.0, 0.0, 64.0, 64.0, 0.0, identity,
        ); // Head
        self.add_box(
            &mut tris, 0, -4.0, -4.0, -2.0, 8.0, 12.0, 4.0, 16.0, 16.0, 64.0, 64.0, 0.0, identity,
        ); // Body

        let rotate_x = |anchor: Vec3, angle: f32| {
            move |v: Vec3| -> Vec3 {
                let dy = v.y - anchor.y;
                let dz = v.z - anchor.z;
                let c = angle.cos();
                let s = angle.sin();
                Vec3::new(v.x, anchor.y + dy * c - dz * s, anchor.z + dy * s + dz * c)
            }
        };

        let r_arm_tx = rotate_x(Vec3::new(-4.0 - arm_w / 2.0, -4.0, 0.0), -arm_angle);
        let l_arm_tx = rotate_x(Vec3::new(4.0 + arm_w / 2.0, -4.0, 0.0), arm_angle);
        let r_leg_tx = rotate_x(Vec3::new(-2.0, 8.0, 0.0), leg_angle);
        let l_leg_tx = rotate_x(Vec3::new(2.0, 8.0, 0.0), -leg_angle);

        self.add_box(
            &mut tris,
            0,
            -4.0 - arm_w,
            -4.0,
            -2.0,
            arm_w,
            12.0,
            4.0,
            40.0,
            16.0,
            64.0,
            64.0,
            0.0,
            r_arm_tx.clone(),
        ); // Right Arm
        self.add_box(
            &mut tris,
            0,
            4.0,
            -4.0,
            -2.0,
            arm_w,
            12.0,
            4.0,
            32.0,
            48.0,
            64.0,
            64.0,
            0.0,
            l_arm_tx.clone(),
        ); // Left Arm

        self.add_box(
            &mut tris,
            0,
            -4.0,
            8.0,
            -2.0,
            4.0,
            12.0,
            4.0,
            0.0,
            16.0,
            64.0,
            64.0,
            0.0,
            r_leg_tx.clone(),
        ); // Right Leg
        self.add_box(
            &mut tris,
            0,
            0.0,
            8.0,
            -2.0,
            4.0,
            12.0,
            4.0,
            16.0,
            48.0,
            64.0,
            64.0,
            0.0,
            l_leg_tx.clone(),
        ); // Left Leg

        let inf = 0.25;
        self.add_box(
            &mut tris, 0, -4.0, -12.0, -4.0, 8.0, 8.0, 8.0, 32.0, 0.0, 64.0, 64.0, inf, identity,
        ); // Head Overlay
        self.add_box(
            &mut tris, 0, -4.0, -4.0, -2.0, 8.0, 12.0, 4.0, 16.0, 32.0, 64.0, 64.0, inf, identity,
        ); // Body Overlay
        self.add_box(
            &mut tris,
            0,
            -4.0 - arm_w,
            -4.0,
            -2.0,
            arm_w,
            12.0,
            4.0,
            40.0,
            32.0,
            64.0,
            64.0,
            inf,
            r_arm_tx,
        ); // Right Arm Overlay
        self.add_box(
            &mut tris, 0, 4.0, -4.0, -2.0, arm_w, 12.0, 4.0, 48.0, 48.0, 64.0, 64.0, inf, l_arm_tx,
        ); // Left Arm Overlay
        self.add_box(
            &mut tris, 0, -4.0, 8.0, -2.0, 4.0, 12.0, 4.0, 0.0, 32.0, 64.0, 64.0, inf, r_leg_tx,
        ); // Right Leg Overlay
        self.add_box(
            &mut tris, 0, 0.0, 8.0, -2.0, 4.0, 12.0, 4.0, 0.0, 48.0, 64.0, 64.0, inf, l_leg_tx,
        ); // Left Leg Overlay

        if self.cape.is_some() {
            let cape_tx = rotate_x(
                Vec3::new(0.0, -4.0, 2.0),
                (walk_anim * std::f32::consts::PI * 2.0).sin() * 0.15 + 0.25,
            );
            self.add_box_cape(
                &mut tris, 1, -5.0, -4.0, 2.0, 10.0, 16.0, 1.0, 0.0, 0.0, 64.0, 32.0, 0.0, cape_tx,
            );
        }

        let cy = yaw.cos();
        let sy = yaw.sin();
        let cp = pitch.cos();
        let sp = pitch.sin();

        let transform = |v: &Vec3| -> Vec3 {
            let x1 = v.x * cy - v.z * sy;
            let z1 = v.x * sy + v.z * cy;
            let y1 = v.y * cp - z1 * sp;
            let z2 = v.y * sp + z1 * cp;

            let scale = height as f32 / 35.0;
            Vec3::new(
                x1 * scale + width as f32 / 2.0,
                (y1 - 4.0) * scale + height as f32 / 2.0,
                z2 * scale,
            )
        };

        let mut projected = Vec::new();
        for tri in tris {
            let p0 = transform(&tri.v0.pos);
            let p1 = transform(&tri.v1.pos);
            let p2 = transform(&tri.v2.pos);

            let normal = p1.sub(&p0).cross(&p2.sub(&p0));
            if normal.z < 0.0 {
                projected.push((
                    p0, tri.v0.u, tri.v0.v, p1, tri.v1.u, tri.v1.v, p2, tri.v2.u, tri.v2.v,
                    tri.v0.tex,
                ));
            }
        }

        let mut z_buffer = vec![f32::MAX; (width * height) as usize];
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let pixels = buffer.make_mut_slice();
        for p in pixels.iter_mut() {
            *p = Rgba8Pixel {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
        }

        let tex_w = self.image.width() as f32;
        let tex_h = self.image.height() as f32;

        for (p0, u0, v0, p1, u1, v1, p2, u2, v2, tex) in projected {
            let min_x = (p0.x.min(p1.x).min(p2.x).max(0.0) as u32).min(width - 1);
            let max_x = (p0.x.max(p1.x).max(p2.x).max(0.0) as u32).min(width - 1);
            let min_y = (p0.y.min(p1.y).min(p2.y).max(0.0) as u32).min(height - 1);
            let max_y = (p0.y.max(p1.y).max(p2.y).max(0.0) as u32).min(height - 1);

            let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
            if denom.abs() < 0.001 {
                continue;
            }

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;

                    let w0 = ((p1.y - p2.y) * (px - p2.x) + (p2.x - p1.x) * (py - p2.y)) / denom;
                    let w1 = ((p2.y - p0.y) * (px - p2.x) + (p0.x - p2.x) * (py - p2.y)) / denom;
                    let w2 = 1.0 - w0 - w1;

                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }

                    let z = w0 * p0.z + w1 * p1.z + w2 * p2.z;
                    let idx = (y * width + x) as usize;
                    if z >= z_buffer[idx] {
                        continue;
                    }

                    let u = w0 * u0 + w1 * u1 + w2 * u2;
                    let v = w0 * v0 + w1 * v1 + w2 * v2;

                    let pixel = if tex == 0 {
                        let tx = (u * tex_w).clamp(0.0, tex_w - 1.0) as u32;
                        let ty = (v * tex_h).clamp(0.0, tex_h - 1.0) as u32;
                        self.image.get_pixel(tx, ty)
                    } else if let Some(cape) = &self.cape {
                        let c_w = cape.width() as f32;
                        let c_h = cape.height() as f32;
                        let tx = (u * c_w).clamp(0.0, c_w - 1.0) as u32;
                        let ty = (v * c_h).clamp(0.0, c_h - 1.0) as u32;
                        cape.get_pixel(tx, ty)
                    } else {
                        &image::Rgba([0, 0, 0, 0])
                    };

                    if pixel[3] == 0 {
                        continue;
                    }

                    if pixel[3] < 255 {
                        let bg = pixels[idx];
                        let alpha = pixel[3] as f32 / 255.0;
                        let inv_alpha = 1.0 - alpha;
                        pixels[idx] = Rgba8Pixel {
                            r: (pixel[0] as f32 * alpha + bg.r as f32 * inv_alpha) as u8,
                            g: (pixel[1] as f32 * alpha + bg.g as f32 * inv_alpha) as u8,
                            b: (pixel[2] as f32 * alpha + bg.b as f32 * inv_alpha) as u8,
                            a: 255,
                        };
                    } else {
                        z_buffer[idx] = z;
                        pixels[idx] = Rgba8Pixel {
                            r: pixel[0],
                            g: pixel[1],
                            b: pixel[2],
                            a: pixel[3],
                        };
                    }
                }
            }
        }

        buffer
    }
}
