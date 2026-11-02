#![allow(dead_code)]

use image::{ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};

pub fn photograph(width: u32, height: u32) -> RgbImage {
    let mut seed = 42_u32;
    RgbImage::from_fn(width, height, |x, y| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        Rgb([
            ((x * 255 / width) as u8).saturating_add((seed % 40) as u8),
            ((y * 255 / height) as u8).saturating_add(((seed >> 8) % 40) as u8),
            ((x + y) % 256) as u8,
        ])
    })
}

pub fn jpeg(quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&photograph(512, 384))
        .unwrap();
    out
}

pub fn png() -> Vec<u8> {
    let img = RgbaImage::from_fn(80, 60, |x, y| {
        Rgba([x as u8, y as u8, 123, if x < 40 { 0 } else { 255 }])
    });
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(img.as_raw(), 80, 60, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

pub fn add_png_chunk(input: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    let mut crc = !0_u32;
    for byte in &chunk[4..] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    chunk.extend_from_slice(&(!crc).to_be_bytes());
    let mut out = input[..33].to_vec();
    out.extend(chunk);
    out.extend_from_slice(&input[33..]);
    out
}

pub fn add_jpeg_exif(input: &[u8], orientation: u16) -> Vec<u8> {
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0; 6]);
    let mut out = input[..2].to_vec();
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    out.extend(exif);
    out.extend_from_slice(&input[2..]);
    out
}
