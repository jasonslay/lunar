use lunar::build_app;

#[cfg(not(target_arch = "wasm32"))]
use lunar::configure_native;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut app = build_app();
    #[cfg(not(target_arch = "wasm32"))]
    configure_native(&mut app);
    app.run();
}
