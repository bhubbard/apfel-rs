use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo::rustc-check-cfg=cfg(has_foundation_models)");
    println!("cargo:rerun-if-changed=bridge/foundation_models_bridge.swift");
    println!("cargo:rerun-if-changed=Info.plist");

    if target_os == "macos" {
        let bridge_swift = manifest_dir.join("bridge/foundation_models_bridge.swift");
        let bridge_obj = out_dir.join("foundation_models_bridge.o");
        let info_plist = manifest_dir.join("Info.plist");

        // Compile Swift bridge into object file
        let status = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-O")
            .arg("-whole-module-optimization")
            .arg("-c")
            .arg(&bridge_swift)
            .arg("-o")
            .arg(&bridge_obj)
            .status();

        match status {
            Ok(s) if s.success() => {
                let lib_path = out_dir.join("libapfel_bridge.a");
                let ar_status = Command::new("ar")
                    .arg("crs")
                    .arg(&lib_path)
                    .arg(&bridge_obj)
                    .status();

                if let Ok(ars) = ar_status {
                    if ars.success() {
                        println!("cargo:rustc-cfg=has_foundation_models");
                        println!("cargo:rustc-link-search=native={}", out_dir.display());
                        println!("cargo:rustc-link-lib=static=apfel_bridge");
                        println!("cargo:rustc-link-arg=-sectcreate");
                        println!("cargo:rustc-link-arg=__TEXT");
                        println!("cargo:rustc-link-arg=__info_plist");
                        println!("cargo:rustc-link-arg={}", info_plist.display());
                        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
                        println!("cargo:rustc-link-arg=-Wl,-rpath,/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx");
                        println!("cargo:rustc-link-search=native=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx");
                        println!("cargo:rustc-link-lib=framework=Foundation");
                        println!("cargo:rustc-link-lib=framework=FoundationModels");
                        println!("cargo:rustc-link-search=framework=/System/Library/Frameworks");
                        println!("cargo:rustc-link-search=native=/usr/lib/swift");
                    }
                }
            }
            _ => {
                println!(
                    "cargo:warning=Failed to compile foundation_models_bridge.swift; using mock backend fallback"
                );
            }
        }
    } else {
        println!("cargo:warning=Building on non-macOS target; using mock backend fallback");
    }
}
