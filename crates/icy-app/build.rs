//! Compiles the embedded JSNIR SDK + icy TSX app using JSNIR's pinned TypeScript 7 compiler.
mod app_bundle;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn resolve_tsc(workspace_root: &Path) -> Result<PathBuf, String> {
    let mut checked = Vec::new();

    if let Some(explicit) = env::var_os("JSNIR_TSC") {
        let raw = PathBuf::from(explicit);
        let candidate = if raw.is_relative() {
            workspace_root.join(&raw)
        } else {
            raw
        };
        if candidate.is_file() {
            return candidate
                .canonicalize()
                .map_err(|e| format!("cannot resolve JSNIR_TSC ({}): {e}", candidate.display()));
        }
        return Err(format!(
            "JSNIR_TSC points to missing file: {}",
            candidate.display()
        ));
    }

    let tsc_bin = if cfg!(windows) { "tsc.exe" } else { "tsc" };
    let fetched = workspace_root
        .join("../Jurui/target/typescript/lib")
        .join(tsc_bin);
    checked.push(fetched.clone());
    if fetched.is_file() {
        return fetched
            .canonicalize()
            .map_err(|e| format!("cannot resolve {}: {e}", fetched.display()));
    }

    let pkg_dir = if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "typescript-win32-arm64"
        } else {
            "typescript-win32-x64"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "typescript-darwin-arm64"
        } else {
            "typescript-darwin-x64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "typescript-linux-arm64"
    } else {
        "typescript-linux-x64"
    };

    let npm_native = workspace_root
        .join("../Jurui/node_modules/@typescript")
        .join(pkg_dir)
        .join("lib")
        .join(tsc_bin);
    checked.push(npm_native.clone());
    if npm_native.is_file() {
        return npm_native
            .canonicalize()
            .map_err(|e| format!("cannot resolve {}: {e}", npm_native.display()));
    }

    let list = checked
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "TypeScript 7 native compiler not found. Checked:\n{list}\nSet JSNIR_TSC or run `npm ci` in ../Jurui."
    ))
}

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = crate_dir
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("cargo:rerun-if-env-changed=JSNIR_TSC");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=app_bundle.rs");

    let compiler = resolve_tsc(&workspace_root).unwrap_or_else(|error| panic!("{error}"));
    println!("cargo:rerun-if-changed={}", compiler.display());

    let bundle = app_bundle::Bundle::new(&workspace_root, compiler)
        .unwrap_or_else(|error| panic!("{error}"));
    for input in bundle.inputs() {
        println!("cargo:rerun-if-changed={}", input.display());
    }

    let script = bundle
        .compile(&out)
        .unwrap_or_else(|error| panic!("{error}"));
    fs::write(out.join("app.js"), script).unwrap();
}
