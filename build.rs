use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/logo.png");

    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR missing");
    let icon_path = Path::new(&out_dir).join("app.ico");
    let rc_path = Path::new(&out_dir).join("app.rc");
    let res_path = Path::new(&out_dir).join("app.res");

    let png = fs::read("assets/logo.png").expect("read assets/logo.png");
    let mut ico = Vec::with_capacity(png.len() + 22);
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.push(0);
    ico.push(0);
    ico.push(0);
    ico.push(0);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&png);
    fs::write(&icon_path, ico).expect("write ico");
    fs::write(
        &rc_path,
        format!(
            "1 ICON \"{}\"\n",
            icon_path.display().to_string().replace('\\', "\\\\")
        ),
    )
    .expect("write rc");

    let windres = env::var("WINDRES").unwrap_or_else(|_| {
        if target.starts_with("x86_64-pc-windows-gnu") {
            "x86_64-w64-mingw32-windres".to_string()
        } else {
            "windres".to_string()
        }
    });

    let status = Command::new(&windres)
        .arg(&rc_path)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&res_path)
        .status()
        .expect("run windres");
    assert!(status.success(), "windres failed");

    println!("cargo:rustc-link-arg={}", res_path.display());
}
