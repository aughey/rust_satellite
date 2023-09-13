fn main() {
    // @aughey ➜ /workspaces/rust_satellite (main) $ RUSTFLAGS="-C link-arg=-Wl,-L/lib/arm-linux-gnueabihf,-ludev" PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig cross build --release --target arm-unknown-linux-gnueabihf 2>&1 | less
    println!("cargo:rustc-link-search=/lib/arm-linux-gnueabihf");

    // include /usr/include
    println!("cargo:include=/usr/include")

   // println!("cargo:rustc-link-search=/usr/lib/arm-linux-gnueabihf");
    // Add -static-libudev to the linker flags
    //println!("cargo:rustc-link-lib=udev");
}
