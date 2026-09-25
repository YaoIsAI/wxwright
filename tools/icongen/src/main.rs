//! icongen: generate the full icon set from assets/icon/master.svg
//! (PRD 3.9-C single-master export pipeline; outputs are never hand-edited).
//!
//! Sharpness strategy (fixes blurry taskbar icon): render ONE large
//! supersample (1024px) from the master SVG, then downsample to every
//! target size with Lanczos resampling. Direct tiny-size rasterization of
//! fine 11%-wide strokes produces muddy antialiasing at 16-32 px.

use std::path::PathBuf;

use image::imageops::FilterType;
use resvg::tiny_skia;
use resvg::usvg;

const SIZES_PNG: &[u32] = &[32, 128, 256, 512];
const SIZES_ICO: &[u32] = &[16, 20, 24, 32, 48, 64, 128, 256];
const SUPERSAMPLE: u32 = 1024;

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
    let big_img = image::RgbaImage::from_raw(
        SUPERSAMPLE,
        SUPERSAMPLE,
        big.data().to_vec(),
    )
    .expect("wrap pixmap");

    let downscale = |size: u32| -> image::RgbaImage {
        image::imageops::resize(&big_img, size, size, FilterType::Lanczos3)
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

    println!(
        "generated {} png files + icon.ico ({} frames, lanczos from {}px) into {}",
        png_paths.len(),
        SIZES_ICO.len(),
        SUPERSAMPLE,
        out_dir.display()
    );
}
