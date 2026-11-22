use super::*;
use std::{fs, io::Write};

fn docx(xml: &str) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut buffer);
    writer
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(xml.as_bytes()).unwrap();
    writer.finish().unwrap();
    buffer.into_inner()
}

const PARAGRAPHS: &str = r#"<w:document xmlns:w="w"><w:body><w:p><w:r><w:t>Bases de datos</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">Normalización &amp; índices: </w:t><w:tab/><w:t>tercera forma normal.</w:t></w:r></w:p></w:body></w:document>"#;

#[test]
fn reads_the_paragraphs_of_a_docx() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Informe.docx");
    fs::write(&path, docx(PARAGRAPHS)).unwrap();
    let extracted = extract(&path).unwrap();
    assert_eq!(
        extracted.text,
        "Bases de datos\nNormalización & índices: tercera forma normal."
    );
    assert!(!extracted.truncated);
}

#[test]
fn reads_plain_text_with_bom_and_trims_long_documents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.md");
    let long = format!("\u{feff}# Notas\n\n\n\n{}", "palabra ".repeat(10_000));
    fs::write(&path, long).unwrap();
    let extracted = extract(&path).unwrap();
    assert!(extracted.text.starts_with("# Notas\n\npalabra"));
    assert_eq!(extracted.text.chars().count(), MAX_DOCUMENT_CHARS);
    assert!(extracted.truncated);
}

#[test]
fn says_clearly_what_cannot_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("foto.png");
    fs::write(&image, [0x89, b'P', b'N', b'G']).unwrap();
    assert_eq!(
        extract(&image),
        Err(DocumentError::Unsupported(".png".into()))
    );
    let broken = dir.path().join("roto.pdf");
    fs::write(&broken, b"%PDF-1.7 esto no es un PDF").unwrap();
    assert_eq!(extract(&broken), Err(DocumentError::Unreadable));
    let fake_docx = dir.path().join("falso.docx");
    fs::write(&fake_docx, b"no es un zip").unwrap();
    assert_eq!(extract(&fake_docx), Err(DocumentError::Unreadable));
    let empty = dir.path().join("vacio.txt");
    fs::write(&empty, "  \n ").unwrap();
    assert_eq!(extract(&empty), Err(DocumentError::Empty));
    assert_eq!(
        extract(&dir.path().join("no-existe.txt")),
        Err(DocumentError::Missing)
    );
    assert!(DocumentError::Scanned.to_string().contains("escaneado"));
}

#[test]
fn reading_does_not_modify_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("informe.txt");
    fs::write(&path, "Un informe con texto suficiente para leerlo.").unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    extract(&path).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    assert!(path.exists());
}

#[test]
fn content_search_only_reads_small_text_files() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("apuntes.txt");
    fs::write(&text, "Modelo entidad-relación y Bases de Datos").unwrap();
    let terms = vec!["bases".to_owned(), "datos".to_owned()];
    assert!(text_contains(&text, &terms, 1024));
    assert!(!text_contains(&text, &terms, 4));
    let pdf = dir.path().join("apuntes.pdf");
    fs::write(&pdf, "bases datos").unwrap();
    assert!(!text_contains(&pdf, &terms, 1024));
}

#[test]
fn only_offered_files_can_be_read() {
    let offers = DocumentOffers::default();
    let token = offers.offer(PathBuf::from("C:/a/informe.pdf"));
    assert_eq!(offers.path(&token), Some(PathBuf::from("C:/a/informe.pdf")));
    assert_eq!(offers.path("C:/a/informe.pdf"), None);
    assert_eq!(offers.path("otro"), None);
}
