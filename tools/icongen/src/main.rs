//! icongen: generate the full icon set from assets/icon/master.svg
//! (PRD 3.9-C single-master export pipeline; outputs are never hand-edited).
//!
//! Sharpness strategy (fixes blurry taskbar icon): render ONE large
//! supersample (1024px) from the master SVG, then downsample to every
//! target size with Lanczos resampling. Direct tiny-size rasterization of
//! fine 11%-wide strokes produces muddy antialiasing at 16-32 px.
//!
//! Platform geometry (docs/icon-sop.md): Windows/Linux want the full-bleed
//! square, but macOS composes app icons on Apple's official template: an
//! 824px continuous-curvature squircle (superellipse, n=5) centered on the
//! 1024 canvas with transparent margins and a soft baked drop shadow. A
//! full-bleed bitmap ships "one size bigger" than every standard Dock/Finder
//! icon. The macOS variant is composed here automatically and written as
//! icon.icns (PNG-chunk container), so an off-spec icns can never ship again.

use std::path::PathBuf;

use image::imageops::FilterType;
use resvg::tiny_skia;
use resvg::usvg;

const SIZES_PNG: &[u32] = &[32, 128, 256, 512];
const SIZES_ICO: &[u32] = &[16, 20, 24, 32, 48, 64, 128, 256];
const SUPERSAMPLE: u32 = 1024;
/// Apple macOS app-icon grid (HIG template): the squircle's edge on the
/// 1024 canvas - 80.5% coverage, transparent margins around it.
const MACOS_TILE: u32 = 824;
/// (icns OSType, pixel size) - the standard set iconutil emits for a 1024
/// source; PNG-based entries only (ic04/ic05 are legacy JPEG-2000 variants).
const SIZES_ICNS: &[(&str, u32)] = &[
    ("icp4", 16),
    ("icp5", 32),
    ("ic11", 32),
    ("ic12", 64),
    ("ic07", 128),
    ("ic13", 256),
    ("ic08", 256),
    ("ic14", 512),
    ("ic09", 512),
    ("ic10", 1024),
];

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master = root.join("assets/icon/master.svg");
    let out_dir = root.join("gui/src-tauri/icons");
    std::fs::create_dir_all(&out_dir).expect("create icons dir");

    let svg = std::fs::read_to_string(&master).expect("read master.svg");
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("parse master.svg");
    let base = tree.size().width();

    // single high-res render, reused for every downsize
    let mut big = tiny_skia::Pixmap::new(SUPERSAMPLE, SUPERSAMPLE).expect("pixmap");
    let scale = SUPERSAMPLE as f32 / base;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut big.as_mut(),
    );
    let big_img = image::RgbaImage::from_raw(SUPERSAMPLE, SUPERSAMPLE, big.data().to_vec())
        .expect("wrap pixmap");

    let downscale = |size: u32| -> image::RgbaImage {
        image::imageops::resize(&big_img, size, size, FilterType::Lanczos3)
    };

    // macOS composition on Apple's official app-icon grid (HIG template):
    //   - the squircle spans 824px on the 1024 canvas (80.5%), margins transparent
    //   - the shape is Apple's continuous-curvature squircle, i.e. the
    //     superellipse |x/a|^n + |y/a|^n = 1 with n = 5 and a = 824/2 - NOT a
    //     rounded rectangle (circular corners visibly deviate at large sizes)
    //   - the template ships with a soft baked drop shadow under the tile
    let macos_master = {
        let a = (MACOS_TILE as f64) / 2.0;
        let n = 5.0f64;
        let center = (MACOS_TILE as f64) / 2.0;
        let inside = |px: f64, py: f64| -> bool {
            let dx = (px - center).abs() / a;
            let dy = (py - center).abs() / a;
            dx.powf(n) + dy.powf(n) <= 1.0
        };
        // 4x4 analytic supersampling per pixel: coverage-accurate, jaggie-free
        // edges without a second render pass
        let mut mask824 = image::RgbaImage::new(MACOS_TILE, MACOS_TILE);
        for y in 0..MACOS_TILE {
            for x in 0..MACOS_TILE {
                let mut hit = 0u32;
                for sy in 0..4u32 {
                    for sx in 0..4u32 {
                        if inside(
                            x as f64 + (sx as f64 + 0.5) / 4.0,
                            y as f64 + (sy as f64 + 0.5) / 4.0,
                        ) {
                            hit += 1;
                        }
                    }
                }
                if hit > 0 {
                    let alpha = (hit * 255 / 16) as u8;
                    mask824.put_pixel(x, y, image::Rgba([255, 255, 255, alpha]));
                }
            }
        }

        let mut canvas = image::RgbaImage::new(SUPERSAMPLE, SUPERSAMPLE);
        let offset = ((SUPERSAMPLE - MACOS_TILE) / 2) as i64;

        // baked template shadow: the mask, black, dimmed and blurred, shifted
        // down - composited first so the tile sits on top of it
        let mut shape1024 = image::RgbaImage::new(SUPERSAMPLE, SUPERSAMPLE);
        image::imageops::overlay(&mut shape1024, &mask824, offset, offset);
        let mut shadow = image::RgbaImage::new(SUPERSAMPLE, SUPERSAMPLE);
        for (x, y, p) in shape1024.enumerate_pixels() {
            let alpha = p[3];
            if alpha > 0 {
                shadow.put_pixel(
                    x,
                    y,
                    image::Rgba([0, 0, 0, (alpha as u32 * 64 / 255) as u8]),
                );
            }
        }
        let shadow = image::imageops::blur(&shadow, 12.0);
        image::imageops::overlay(&mut canvas, &shadow, 0, 14);

        // the tile: full-bleed master scaled onto the grid, its corners cut
        // by the squircle mask (master corner radius is masked away entirely)
        let mut tile =
            image::imageops::resize(&big_img, MACOS_TILE, MACOS_TILE, FilterType::Lanczos3);
        for (x, y, p) in tile.enumerate_pixels_mut() {
            let m = mask824.get_pixel(x, y)[3];
            p[3] = ((p[3] as u32 * m as u32) / 255) as u8;
        }
        image::imageops::overlay(&mut canvas, &tile, offset, offset);
        canvas
    };
    let downscale_macos = |size: u32| -> image::RgbaImage {
        image::imageops::resize(&macos_master, size, size, FilterType::Lanczos3)
    };

    let encode_png = |img: &image::RgbaImage| -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode png");
        buf.into_inner()
    };

    let mut png_paths: Vec<(u32, PathBuf)> = Vec::new();
    for &size in SIZES_PNG {
        let name = match size {
            32 => "32x32.png",
            128 => "128x128.png",
            256 => "128x128@2x.png",
            512 => "icon.png",
            _ => unreachable!(),
        };
        let path = out_dir.join(name);
        std::fs::write(&path, encode_png(&downscale(size))).expect("write png");
        png_paths.push((size, path));
    }

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in SIZES_ICO {
        let img = ico::IconImage::from_rgba_data(size, size, downscale(size).into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&img).expect("encode ico entry"));
    }
    let ico_path = out_dir.join("icon.ico");
    let file = std::fs::File::create(&ico_path).expect("create ico");
    dir.write(std::io::BufWriter::new(file)).expect("write ico");

    // icon.icns: PNG-chunk container ('icns' + u32 BE total size, then
    // per entry the 4-byte OSType, u32 BE chunk size, PNG bytes)
    let mut icns: Vec<u8> = Vec::new();
    icns.extend_from_slice(b"icns");
    let total = 8 + SIZES_ICNS
        .iter()
        .map(|(_, size)| 8 + encode_png(&downscale_macos(*size)).len() as u32)
        .sum::<u32>();
    icns.extend_from_slice(&total.to_be_bytes());
    for (os_type, size) in SIZES_ICNS {
        let png = encode_png(&downscale_macos(*size));
        icns.extend_from_slice(os_type.as_bytes());
        icns.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
        icns.extend_from_slice(&png);
    }
    std::fs::write(out_dir.join("icon.icns"), &icns).expect("write icns");

    println!(
        "generated {} png files + icon.ico ({} frames) + icon.icns ({} entries, \
         macOS tile {MACOS_TILE}/{SUPERSAMPLE}) into {}",
        png_paths.len(),
        SIZES_ICO.len(),
        SIZES_ICNS.len(),
        out_dir.display(),
    );
}
