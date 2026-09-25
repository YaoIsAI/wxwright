//! icongen: generate the full icon set from assets/icon/master.svg
//! (PRD 3.9-C single-master export pipeline; outputs are never hand-edited).

use std::path::PathBuf;

use resvg::tiny_skia;
use resvg::usvg;

const SIZES_PNG: &[u32] = &[32, 128, 256, 512];
const SIZES_ICO: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master = root.join("assets/icon/master.svg");
    let out_dir = root.join("gui/src-tauri/icons");
    std::fs::create_dir_all(&out_dir).expect("create icons dir");

    let svg = std::fs::read_to_string(&master).expect("read master.svg");
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("parse master.svg");
    let base = tree.size().width();

    let render = |size: u32| -> Vec<u8> {
        let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
        let scale = size as f32 / base;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        pixmap.encode_png().expect("encode png")
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
        std::fs::write(&path, render(size)).expect("write png");
        png_paths.push((size, path));
    }

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in SIZES_ICO {
        let png = render(size);
        let img = ico::IconImage::read_png(std::io::Cursor::new(png)).expect("read png for ico");
        dir.add_entry(ico::IconDirEntry::encode(&img).expect("encode ico entry"));
    }
    let ico_path = out_dir.join("icon.ico");
    let file = std::fs::File::create(&ico_path).expect("create ico");
    dir.write(std::io::BufWriter::new(file)).expect("write ico");

    println!(
        "generated {} png files + icon.ico into {}",
        png_paths.len(),
        out_dir.display()
    );
}
