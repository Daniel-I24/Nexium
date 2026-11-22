//! Lectura local de documentos para resumirlos o analizarlos. Solo se lee: nada se modifica, mueve ni elimina.
//!
//! Formatos: texto (`.txt`, `.md`, `.csv`), PDF con texto y Word (`.docx`). El texto extraído se recorta a
//! `MAX_DOCUMENT_CHARS` antes de enviarse a ningún sitio, y solo tras la confirmación expresa de la persona.

use std::{
    collections::HashMap,
    io::{Cursor, Read},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::Mutex,
};

use quick_xml::{events::Event, Reader};
use uuid::Uuid;

/// Extensiones que Nexium sabe leer.
pub const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md", "csv", "pdf", "docx"];
/// Formatos de texto plano: en estos también se busca por contenido.
pub const TEXT_EXTENSIONS: &[&str] = &["txt", "md", "csv"];
/// Un archivo mayor no se abre: leerlo tardaría y su texto no cabría en el análisis.
pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
/// Lo que se envía a la IA como máximo: unas 15 páginas de texto. El resto no sale del equipo.
pub const MAX_DOCUMENT_CHARS: usize = 30_000;
/// Un `.docx` descomprimido mayor que esto se rechaza (protege frente a archivos manipulados).
const MAX_DOCX_XML_BYTES: u64 = 50 * 1024 * 1024;
/// Con menos texto que esto, un PDF casi seguro es una imagen escaneada.
const MIN_TEXT_CHARS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    Missing,
    Unsupported(String),
    TooLarge,
    Unreadable,
    Scanned,
    Empty,
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Missing => "El archivo ya no está en su carpeta.".to_owned(),
            Self::Unsupported(extension) => format!(
                "No sé leer archivos {extension}. Puedo leer .txt, .md, .csv, .pdf con texto y .docx."
            ),
            Self::TooLarge => "El archivo pasa de 20 MB: es demasiado grande para leerlo.".into(),
            Self::Unreadable => {
                "No pude leer el archivo: puede estar dañado o protegido con contraseña.".into()
            }
            Self::Scanned => "El PDF no tiene texto que leer: parece un documento escaneado, una imagen de las páginas. Nexium no reconoce texto en imágenes.".into(),
            Self::Empty => "El archivo no tiene texto.".into(),
        };
        f.write_str(&message)
    }
}

/// Texto listo para enviar, ya recortado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub text: String,
    /// Si se recortó a `MAX_DOCUMENT_CHARS`.
    pub truncated: bool,
}

pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_supported(path: &Path) -> bool {
    SUPPORTED_EXTENSIONS.contains(&extension_of(path).as_str())
}

/// Lee el texto de un documento. Pensado para un hilo de trabajo: puede tardar en un PDF largo.
pub fn extract(path: &Path) -> Result<Extracted, DocumentError> {
    let extension = extension_of(path);
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(DocumentError::Unsupported(if extension.is_empty() {
            "sin extensión".into()
        } else {
            format!(".{extension}")
        }));
    }
    let metadata = std::fs::metadata(path).map_err(|_| DocumentError::Missing)?;
    if !metadata.is_file() {
        return Err(DocumentError::Missing);
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(DocumentError::TooLarge);
    }
    let bytes = std::fs::read(path).map_err(|_| DocumentError::Unreadable)?;
    let text = match extension.as_str() {
        "pdf" => pdf_text(&bytes)?,
        "docx" => docx_text(&bytes)?,
        _ => plain_text(&bytes),
    };
    let text = tidy(&text);
    if text.chars().filter(|c| !c.is_whitespace()).count() < MIN_TEXT_CHARS {
        return Err(if extension == "pdf" {
            DocumentError::Scanned
        } else {
            DocumentError::Empty
        });
    }
    Ok(truncate(text))
}

/// Texto plano en UTF-8 (con o sin BOM). Lo que no sea UTF-8 válido se sustituye, nunca falla.
pub fn plain_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Un PDF malformado puede hacer fallar al lector con un pánico: se contiene y se informa como ilegible.
fn pdf_text(bytes: &[u8]) -> Result<String, DocumentError> {
    catch_unwind(AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(bytes)
    }))
    .map_err(|_| DocumentError::Unreadable)?
    .map_err(|_| DocumentError::Unreadable)
}

/// Un `.docx` es un ZIP: el texto está en `word/document.xml`, en elementos `w:t` dentro de párrafos `w:p`.
fn docx_text(bytes: &[u8]) -> Result<String, DocumentError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| DocumentError::Unreadable)?;
    let mut entry = archive
        .by_name("word/document.xml")
        .map_err(|_| DocumentError::Unreadable)?;
    if entry.size() > MAX_DOCX_XML_BYTES {
        return Err(DocumentError::TooLarge);
    }
    let mut xml = String::new();
    entry
        .by_ref()
        .take(MAX_DOCX_XML_BYTES)
        .read_to_string(&mut xml)
        .map_err(|_| DocumentError::Unreadable)?;
    docx_xml_text(&xml)
}

pub fn docx_xml_text(xml: &str) -> Result<String, DocumentError> {
    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if element.name().into_inner() == "w:t" => in_text = true,
            Ok(Event::End(element)) => match element.name().into_inner() {
                "w:t" => in_text = false,
                "w:p" => text.push('\n'),
                _ => {}
            },
            Ok(Event::Empty(element)) => match element.name().into_inner() {
                "w:tab" => text.push('\t'),
                "w:br" | "w:cr" => text.push('\n'),
                _ => {}
            },
            // El texto llega sin entidades: cada `&amp;` o `&#233;` es un evento aparte.
            Ok(Event::Text(content)) if in_text => text.push_str(content.as_ref()),
            Ok(Event::GeneralRef(reference)) if in_text => {
                if let Ok(Some(character)) = reference.resolve_char_ref() {
                    text.push(character);
                } else if let Some(value) =
                    quick_xml::escape::resolve_predefined_entity(reference.as_ref())
                {
                    text.push_str(value);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(DocumentError::Unreadable),
            _ => {}
        }
    }
    Ok(text)
}

/// Sin espacios repetidos ni más de una línea en blanco seguida.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank = 0;
    for line in text.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.trim().to_owned()
}

fn truncate(text: String) -> Extracted {
    match text.char_indices().nth(MAX_DOCUMENT_CHARS) {
        Some((cut, _)) => Extracted {
            text: text[..cut].to_owned(),
            truncated: true,
        },
        None => Extracted {
            text,
            truncated: false,
        },
    }
}

/// Busca los términos en un archivo de texto plano. Solo lee archivos pequeños; el resto se omite.
pub fn text_contains(path: &Path, terms: &[String], max_bytes: u64) -> bool {
    if !TEXT_EXTENSIONS.contains(&extension_of(path).as_str()) {
        return false;
    }
    if !std::fs::metadata(path).is_ok_and(|meta| meta.len() <= max_bytes) {
        return false;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let content = crate::core::rules::normalize(&plain_text(&bytes));
    let needed = terms.len().min(2);
    terms
        .iter()
        .filter(|term| content.contains(&crate::core::rules::normalize(term)))
        .count()
        >= needed.max(1)
}

/// Archivos ofrecidos a la persona para resumir. La interfaz solo conoce un identificador: nunca envía rutas,
/// así que solo se puede leer un archivo que Nexium encontró en una carpeta autorizada o que se eligió en el
/// selector del sistema.
#[derive(Default)]
pub struct DocumentOffers {
    offers: Mutex<HashMap<String, PathBuf>>,
}

/// Ofertas que se recuerdan a la vez. Las más antiguas se olvidan.
const MAX_OFFERS: usize = 50;

impl DocumentOffers {
    pub fn offer(&self, path: PathBuf) -> String {
        let token = Uuid::new_v4().to_string();
        let mut offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if offers.len() >= MAX_OFFERS {
            offers.clear();
        }
        offers.insert(token.clone(), path);
        token
    }

    pub fn path(&self, token: &str) -> Option<PathBuf> {
        self.offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(token)
            .cloned()
    }
}

#[cfg(test)]
#[path = "tests/documents.rs"]
mod tests;
