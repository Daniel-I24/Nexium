//! El tamaño de la ventana de tauri.conf.json coincide con el que usa el código para ajustarla a la pantalla.

use nexium_lib::platform::window_size::{MINIMUM, PREFERRED};

#[test]
fn the_configured_window_sizes_match_the_code() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let window = &config["app"]["windows"][0];
    let number = |key: &str| window[key].as_f64().unwrap();
    assert_eq!((number("width"), number("height")), PREFERRED);
    assert_eq!((number("minWidth"), number("minHeight")), MINIMUM);
}
