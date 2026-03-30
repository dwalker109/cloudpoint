fn main() {
    let target = std::env::var("TARGET").unwrap();

    if target == "armv6k-nintendo-3ds" {
        // Use the pre-built portlib
        println!("cargo:rustc-link-search=native=/opt/devkitpro/portlibs/3ds/lib");
        println!("cargo:rustc-link-lib=static=z");
        println!("cargo:root=/opt/devkitpro/portlibs/3ds");
        println!("cargo:include=/opt/devkitpro/portlibs/3ds/include");
    } else {
        // Use real libz-sys behaviour
        println!("cargo:rustc-link-lib=z");
        println!("cargo:include=/usr/include");
        println!("cargo:root=/usr");
    }
}
