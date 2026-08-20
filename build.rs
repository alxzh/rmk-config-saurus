//! Build script for Saurus keyboard firmware.
//!
//! Copies memory.x linker script, compresses vial.json for Vial support,
//! and sets linker flags.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{env, fs};

use const_gen::*;
use xz2::read::XzEncoder;

fn main() {
    println!("cargo:rerun-if-changed=keyboard.toml");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    generate_macro_keyboard_config(&out);

    // Generate vial config at the root of project
    println!("cargo:rerun-if-changed=vial.json");
    generate_vial_config();

    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory.x");

    // Linker arguments
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
}

/// Give RMK's build-time constants the real battery pins while preventing its
/// config macros from also generating an ADC task. `cargo:rustc-env` is scoped
/// to this package, so RMK's dependencies read the original config while the
/// macros expanding our binaries read this filtered copy. The XIAO needs the
/// custom ADC setup in `src/xiao.rs` for its high-impedance divider.
fn generate_macro_keyboard_config(out: &Path) {
    let config = fs::read_to_string("keyboard.toml").expect("Cannot read keyboard.toml");
    let mut macro_config = String::with_capacity(config.len());
    let mut in_split_board = false;
    let mut removed_pins = 0;

    for line in config.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_split_board = trimmed == "[split.central]" || trimmed == "[[split.peripheral]]";
        }

        if in_split_board && trimmed.starts_with("battery_adc_pin =") {
            removed_pins += 1;
            continue;
        }

        macro_config.push_str(line);
    }

    assert!(
        removed_pins > 0,
        "keyboard.toml must declare split battery_adc_pin metadata"
    );

    let path = out.join("keyboard-macro.toml");
    fs::write(&path, macro_config).expect("Cannot write macro keyboard config");
    println!("cargo:rustc-env=KEYBOARD_TOML_PATH={}", path.display());
}

fn generate_vial_config() {
    let out_file = Path::new(&env::var_os("OUT_DIR").unwrap()).join("config_generated.rs");

    let p = Path::new("vial.json");
    let mut content = String::new();
    match File::open(p) {
        Ok(mut file) => {
            file.read_to_string(&mut content)
                .expect("Cannot read vial.json");
        }
        Err(e) => println!("Cannot find vial.json {:?}: {}", p, e),
    };

    let vial_cfg = json::stringify(json::parse(&content).unwrap());
    let mut keyboard_def_compressed: Vec<u8> = Vec::new();
    XzEncoder::new(vial_cfg.as_bytes(), 6)
        .read_to_end(&mut keyboard_def_compressed)
        .unwrap();

    let keyboard_id: Vec<u8> = vec![0xD8, 0x6D, 0xBE, 0x47, 0x8A, 0x0E, 0x21, 0xC5];
    let const_declarations = [
        const_declaration!(pub VIAL_KEYBOARD_DEF = keyboard_def_compressed),
        const_declaration!(pub VIAL_KEYBOARD_ID = keyboard_id),
    ]
    .map(|s| "#[allow(clippy::redundant_static_lifetimes)]\n".to_owned() + s.as_str())
    .join("\n");
    fs::write(out_file, const_declarations).unwrap();
}
