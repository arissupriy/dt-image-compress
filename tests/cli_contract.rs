#![cfg(feature = "cli")]
mod common;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dt-img"))
}

#[test]
fn produces_decodable_output_without_overwriting_any_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.jpg");
    let original = common::jpeg(98);
    fs::write(&input, &original).unwrap();
    let result = command()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args(["--target", "20KB"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    assert!(String::from_utf8_lossy(&result.stderr).contains("search=Some(LogSecant)"));
    assert!(bytes.len() <= 20_000);
    image::load_from_memory(&bytes).unwrap();
    let second = command()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(fs::read(&input).unwrap(), original);
    let same = command()
        .arg(&input)
        .arg("-o")
        .arg(&input)
        .output()
        .unwrap();
    assert!(!same.status.success());
    assert_eq!(fs::read(&input).unwrap(), original);
}

#[test]
fn failed_target_does_not_create_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.jpg");
    fs::write(&input, common::jpeg(95)).unwrap();
    let result = command()
        .arg(input)
        .arg("-o")
        .arg(&output)
        .args(["--target", "10B"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn stdin_stdout_contains_only_binary_image() {
    let mut child = command()
        .args(["-", "-o", "-", "--target", "20KiB"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&common::jpeg(95))
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.len() <= 20 * 1024);
    assert!(out.stdout.starts_with(&[0xff, 0xd8]));
    image::load_from_memory(&out.stdout).unwrap();
}

#[test]
fn invalid_units_are_rejected_and_png_defaults_to_lossless() {
    let invalid = command()
        .args(["-", "-o", "-", "--target", "200kb"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");
    fs::write(&input, common::png()).unwrap();
    let result = command()
        .arg(input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        image::load_from_memory(&fs::read(output).unwrap())
            .unwrap()
            .to_rgba8(),
        image::load_from_memory(&common::png()).unwrap().to_rgba8()
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_while_waiting_for_stdin_without_eof() {
    let mut child = command()
        .args(["-", "-o", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        Command::new("kill")
            .args(["-s", "INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    for _ in 0..100 {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(130));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!("Ctrl+C did not cancel a blocked stdin reader");
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_blocked_stdout_without_a_reader() {
    use image::ImageEncoder;
    use std::io::Read;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("noise.png");
    let mut seed = 123_u32;
    let pixels = image::RgbImage::from_fn(768, 768, |_, _| {
        image::Rgb(std::array::from_fn(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 24) as u8
        }))
    });
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(pixels.as_raw(), 768, 768, image::ExtendedColorType::Rgb8)
        .unwrap();
    fs::write(&input, png).unwrap();
    let mut child = command()
        .arg(input)
        .args(["-o", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut [0_u8; 1])
        .unwrap();
    assert!(
        Command::new("kill")
            .args(["-s", "INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    for _ in 0..100 {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(130));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!("Ctrl+C did not cancel a blocked stdout writer");
}
