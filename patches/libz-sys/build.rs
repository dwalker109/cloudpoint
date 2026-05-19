fn main() {
    let target = std::env::var("TARGET").unwrap();

    if target == "armv6k-nintendo-3ds" {
        // Use the pre-built portlib
        let devkitpro = std::env::var("DEVKITPRO").expect("DEVKITPRO not set");
        let portlibs = format!("{devkitpro}/portlibs/3ds");
        println!("cargo:rustc-link-search=native={portlibs}/lib");
        println!("cargo:rustc-link-lib=static=z");
        println!("cargo:root={portlibs}");
        println!("cargo:include={portlibs}/include");
    } else {
        // Use real libz-sys behaviour
        println!("cargo:rustc-link-lib=z");
        println!("cargo:include=/usr/include");
        println!("cargo:root=/usr");
    }
}
