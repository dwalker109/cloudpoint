fn main() {
    let portlibs = "/opt/devkitpro/portlibs/3ds/lib";
    println!("cargo:rustc-link-search=native={portlibs}");
    println!("cargo:rustc-link-lib=static=mbedtls");
    println!("cargo:rustc-link-lib=static=mbedx509");
    println!("cargo:rustc-link-lib=static=mbedcrypto");
    println!("cargo:rustc-link-lib=static=z");
    println!("cargo:rerun-if-changed=build.rs");
}
