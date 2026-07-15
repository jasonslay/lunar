fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target != "wasm32" {
        // Embedded before main so Wayland/wgpu startup logs respect the filter.
        println!("cargo:rustc-env=RUST_LOG=info,wgpu=error,naga=warn,wgpu_hal=error,calloop=error");
    }
}
