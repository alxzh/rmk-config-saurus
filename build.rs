//! Build script for Saurus keyboard firmware.
//!
//! Prepares the macro configuration, copies the memory.x linker script, and
//! sets linker flags.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=keyboard.toml");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    generate_macro_keyboard_config(&out);

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
