fn main() {
    citro2d();
    https_curl();

    println!("cargo:rerun-if-changed=build.rs");
}

fn citro2d() {
    let devkitpro = std::env::var("DEVKITPRO").expect("DEVKITPRO not set");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={manifest}/src/ctr_gfx/c2d");
    println!("cargo:rustc-link-search=native={devkitpro}/libctru/lib");
    println!("cargo:rustc-link-arg=-Wl,--start-group");
    println!("cargo:rustc-link-arg=-Wl,-lextern");
    println!("cargo:rustc-link-arg=-Wl,-lcitro2d");
    println!("cargo:rustc-link-arg=-Wl,-lcitro3d");
    println!("cargo:rustc-link-arg=-Wl,-lctru");
    println!("cargo:rustc-link-arg=-Wl,--end-group");
}

fn https_curl() {
    let portlibs = "/opt/devkitpro/portlibs/3ds/lib";
    println!("cargo:rustc-link-search=native={portlibs}");
    println!("cargo:rustc-link-lib=static=mbedtls");
    println!("cargo:rustc-link-lib=static=mbedx509");
    println!("cargo:rustc-link-lib=static=mbedcrypto");
    println!("cargo:rustc-link-lib=static=z");
}
