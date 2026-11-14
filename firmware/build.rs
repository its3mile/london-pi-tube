//! SPDX-License-Identifier: MIT OR Apache-2.0
//!
//! Copyright (c) 2021–2024 The rp-rs Developers
//! Copyright (c) 2021 rp-rs organization
//! Copyright (c) 2025 Raspberry Pi Ltd.
//!
//! Set up linker scripts

use std::fs::{ File, read_to_string };
use std::io::Write;
use std::path::PathBuf;

use regex::Regex;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rp2350)");

    // Put the linker script somewhere the linker can find it
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out.display());

    // `.pico-rs` and `.cargo/` live at the workspace root, where the Raspberry
    // Pi Pico VS Code extension expects them.
    let workspace_root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    let pico_rs = workspace_root.join(".pico-rs");
    println!("cargo:rerun-if-changed={}", pico_rs.display());
    let contents = read_to_string(&pico_rs)
        .map(|s| s.trim().to_string().to_lowercase())
        .unwrap_or_else(|e| {
            eprintln!("Failed to read file: {}", e);
            String::new()
        });

    // The file `memory.x` is loaded by cortex-m-rt's `link.x` script, which
    // is what we specify in `.cargo/config.toml` for Arm builds
    let target;
    if contents.contains("riscv") {
        target = "riscv32imac-unknown-none-elf";
    } else {
        target = "thumbv8m.main-none-eabihf";
    }
    let memory_x = include_bytes!("rp2350.x");
    let mut f = File::create(out.join("memory.x")).unwrap();
    f.write_all(memory_x).unwrap();
    println!("cargo::rustc-cfg=rp2350");
    println!("cargo:rerun-if-changed=rp2350.x");

    let re = Regex::new(r"target = .*").unwrap();
    let config_toml = include_str!("../.cargo/config.toml");
    let result = re.replace(config_toml, format!("target = \"{}\"", target));
    let mut f = File::create(workspace_root.join(".cargo/config.toml")).unwrap();
    f.write_all(result.as_bytes()).unwrap();

    // The file `rp2350_riscv.x` is what we specify in `.cargo/config.toml` for
    // RISC-V builds
    let rp2350_riscv_x = include_bytes!("rp2350_riscv.x");
    let mut f = File::create(out.join("rp2350_riscv.x")).unwrap();
    f.write_all(rp2350_riscv_x).unwrap();
    println!("cargo:rerun-if-changed=rp2350_riscv.x");

    println!("cargo:rerun-if-changed=build.rs");
}
