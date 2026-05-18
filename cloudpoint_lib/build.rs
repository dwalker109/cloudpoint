use std::env;

fn main() {
    https_curl();
    println!("cargo:rerun-if-changed=build.rs");
}

fn https_curl() {
    let devkitpro = std::env::var("DEVKITPRO").expect("DEVKITPRO not set");
    let portlibs = format!("{devkitpro}/portlibs/3ds/lib");
    println!("cargo:rustc-link-search=native={portlibs}");
    println!("cargo:rustc-link-lib=static=mbedtls");
    println!("cargo:rustc-link-lib=static=mbedx509");
    println!("cargo:rustc-link-lib=static=mbedcrypto");
    println!("cargo:rustc-link-lib=static=z");
}
