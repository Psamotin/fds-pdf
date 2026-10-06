//! Deterministic icon resources; source artwork is never changed.
use anyhow::{Context, Result, ensure};
use image::{ImageFormat, imageops::FilterType};
use std::io::Cursor;

pub fn run(args: &[String]) -> Result<()> {
    ensure!(args.is_empty() || args == ["--check"], "usage: cargo xtask fds-icon [--check]");
    let root = super::gates::root().join("assets/fds-pdf");
    let source = image::open(root.join("FDS-PDF.png")).context("read approved FDS PNG")?;
    let mut frames = Vec::new();
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let resized = source.resize(size, size, FilterType::Lanczos3).to_rgba8();
        let mut square = image::RgbaImage::new(size, size);
        image::imageops::overlay(&mut square, &resized, ((size - resized.width()) / 2).into(), ((size - resized.height()) / 2).into());
        let mut bytes = Cursor::new(Vec::new());
        square.write_to(&mut bytes, ImageFormat::Png)?;
        frames.push((size, bytes.into_inner()));
    }
    let mut ico = vec![0, 0, 1, 0, 7, 0];
    let mut offset = 6 + frames.len() as u32 * 16;
    for (size, bytes) in &frames {
        ico.extend([*size as u8, *size as u8, 0, 0, 1, 0, 32, 0]);
        ico.extend((bytes.len() as u32).to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += bytes.len() as u32;
    }
    for (_, bytes) in &frames {
        ico.extend(bytes);
    }
    let window = frames.last().context("missing 256px frame")?.1.clone();
    for (name, data) in [("fds-pdf.ico", ico), ("fds-pdf-window.png", window)] {
        let path = root.join(name);
        if args.is_empty() {
            std::fs::write(&path, data)?;
        } else {
            ensure!(std::fs::read(&path)? == data, "{name} is stale; run cargo xtask fds-icon");
        }
    }
    println!("FDS icon: source preserved, seven ICO sizes and 256px window PNG verified");
    Ok(())
}
