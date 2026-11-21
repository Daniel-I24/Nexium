//! Coherencia de los textos que ve la persona: las citas van entre comillas angulares («»), como en el resto de la
//! app, nunca entre comillas inglesas.

use std::path::Path;

fn sources(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("carpeta") {
        let path = entry.expect("entrada").path();
        if path.is_dir() {
            // Las pruebas pueden citar textos de terceros tal cual.
            if path.file_name().is_some_and(|name| name != "tests") {
                sources(&path, files);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn messages_quote_with_angle_quotes() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let english = ['\u{201c}', '\u{201d}'];
    let escaped = ["\u{201c}", "\u{201d}"];
    let offenders: Vec<_> = files
        .iter()
        .filter(|file| {
            let text = std::fs::read_to_string(file).unwrap_or_default();
            text.contains(english) || escaped.iter().any(|code| text.contains(code))
        })
        .collect();
    assert!(offenders.is_empty(), "{offenders:?}");
}
