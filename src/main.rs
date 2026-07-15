use lunar::build_app;

#[cfg(not(target_arch = "wasm32"))]
use lunar::configure_native;

fn main() {
    let mut app = build_app();
    #[cfg(not(target_arch = "wasm32"))]
    configure_native(&mut app);
    app.run();
}
