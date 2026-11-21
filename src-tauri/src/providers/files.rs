//! Búsqueda en las carpetas autorizadas: por nombre en todos los archivos y, en los de texto plano pequeños,
//! también por contenido. Solo lee: nunca modifica, mueve ni elimina nada.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use crate::{
    core::rules::normalize,
    models::{FileQuery, FileResult},
    providers::documents::text_contains,
};

const MAX_DEPTH: usize = 8;
const MAX_RESULTS: usize = 30;
const MAX_ENTRIES: usize = 200_000;
/// Búsqueda por contenido: solo archivos de texto de hasta 1 MB, como mucho 50 MB leídos y 4 s en total.
/// Pasado el límite, la búsqueda sigue solo por nombre.
const CONTENT_MAX_FILE_BYTES: u64 = 1024 * 1024;
const CONTENT_MAX_TOTAL_BYTES: u64 = 50 * 1024 * 1024;
const CONTENT_MAX_TIME: Duration = Duration::from_secs(4);
const SKIPPED_DIRECTORIES: &[&str] = &["node_modules", "target", ".git", "__pycache__", ".venv"];

#[derive(Debug, Clone)]
pub struct AuthorizedFolder {
    pub path: PathBuf,
    pub label: String,
}

/// Las carpetas guardadas en Ajustes, con su nombre: "Documentos" o "Escritorio" si son esas, y si no, el
/// nombre de la propia carpeta.
pub fn authorized_folders(paths: &[String], known: &[AuthorizedFolder]) -> Vec<AuthorizedFolder> {
    paths
        .iter()
        .map(|path| {
            let path = PathBuf::from(path);
            let label = known
                .iter()
                .find(|folder| folder.path == path)
                .map(|folder| folder.label.clone())
                .or_else(|| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            AuthorizedFolder { path, label }
        })
        .collect()
}

#[derive(Debug, Default, PartialEq)]
pub struct SearchOutcome {
    pub files: Vec<FileResult>,
    pub truncated: bool,
}

#[async_trait]
pub trait FileSearchProvider: Send + Sync {
    async fn search(
        &self,
        query: FileQuery,
        folders: Vec<AuthorizedFolder>,
        cancel: CancellationToken,
    ) -> SearchOutcome;
}

pub struct LocalFileSearch;

fn matches_needed(terms: usize) -> usize {
    terms.min(2)
}

fn score(stem: &str, query: &FileQuery) -> usize {
    if let Some(exact) = &query.exact_name {
        return usize::from(stem == normalize(exact));
    }
    let hits = query
        .terms
        .iter()
        .filter(|term| stem.contains(&normalize(term)))
        .count();
    if hits >= matches_needed(query.terms.len()) {
        hits
    } else {
        0
    }
}

/// Lo que queda para buscar dentro de los archivos.
struct ContentBudget {
    deadline: Instant,
    bytes_left: u64,
}

impl ContentBudget {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + CONTENT_MAX_TIME,
            bytes_left: CONTENT_MAX_TOTAL_BYTES,
        }
    }

    /// Busca los términos en el archivo si queda presupuesto, y lo descuenta.
    fn matches(&mut self, path: &Path, terms: &[String]) -> bool {
        if Instant::now() > self.deadline {
            return false;
        }
        let size = std::fs::metadata(path).map_or(u64::MAX, |meta| meta.len());
        if size > CONTENT_MAX_FILE_BYTES || size > self.bytes_left {
            return false;
        }
        self.bytes_left -= size;
        text_contains(path, terms, CONTENT_MAX_FILE_BYTES)
    }
}

fn walk(
    folder: &AuthorizedFolder,
    query: &FileQuery,
    cancelled: &AtomicBool,
    found: &mut Vec<(usize, FileResult)>,
    entries: &mut usize,
    budget: &mut ContentBudget,
) -> bool {
    let Ok(root) = folder.path.canonicalize() else {
        return false;
    };
    let walker = WalkDir::new(&root)
        .max_depth(MAX_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            entry.depth() == 0
                || !(entry.file_type().is_dir()
                    && (name.starts_with('.') || SKIPPED_DIRECTORIES.contains(&name.as_ref())))
        });
    for entry in walker.filter_map(Result::ok) {
        *entries += 1;
        if *entries > MAX_ENTRIES || cancelled.load(Ordering::Relaxed) {
            return true;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        // Sin seguir enlaces no se puede salir de la raíz, pero se comprueba de todos modos.
        if !path.starts_with(&root) {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let compared = if query.exact_name.is_some() {
            normalize(&file_name)
        } else {
            normalize(&stem_of(path))
        };
        let points = score(&compared, query);
        if points > 0 {
            found.push((points, result_for(path, folder, false)));
        } else if query.exact_name.is_none() && budget.matches(path, &query.terms) {
            // Lo encontrado por contenido va detrás de lo encontrado por nombre.
            found.push((0, result_for(path, folder, true)));
        }
    }
    false
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

fn result_for(path: &Path, folder: &AuthorizedFolder, in_content: bool) -> FileResult {
    let display = path
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned();
    FileResult {
        name: stem_of(path),
        extension: path.extension().map_or_else(String::new, |extension| {
            format!(".{}", extension.to_string_lossy())
        }),
        location: folder.label.clone(),
        path: display,
        exists: path.exists(),
        matched_in_content: in_content,
    }
}

fn run_search(
    query: &FileQuery,
    folders: &[AuthorizedFolder],
    cancelled: &AtomicBool,
) -> SearchOutcome {
    let mut found = Vec::new();
    let mut entries = 0;
    let mut truncated = false;
    let mut budget = ContentBudget::new();
    for folder in folders {
        truncated |= walk(
            folder,
            query,
            cancelled,
            &mut found,
            &mut entries,
            &mut budget,
        );
    }
    // Más términos en el nombre primero y, a igualdad, el nombre más corto: es el más específico.
    found.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.name.len().cmp(&b.1.name.len()))
            .then_with(|| a.1.path.cmp(&b.1.path))
    });
    // Lo encontrado por contenido solo se muestra si ningún nombre coincide: si no, es ruido.
    if found.iter().any(|(points, _)| *points > 0) {
        found.retain(|(points, _)| *points > 0);
    }
    truncated |= found.len() > MAX_RESULTS;
    SearchOutcome {
        files: found
            .into_iter()
            .take(MAX_RESULTS)
            .map(|(_, file)| file)
            .collect(),
        truncated,
    }
}

#[async_trait]
impl FileSearchProvider for LocalFileSearch {
    async fn search(
        &self,
        query: FileQuery,
        folders: Vec<AuthorizedFolder>,
        cancel: CancellationToken,
    ) -> SearchOutcome {
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let task = tokio::task::spawn_blocking(move || run_search(&query, &folders, &flag));
        tokio::select! {
            outcome = task => outcome.unwrap_or_default(),
            () = cancel.cancelled() => {
                cancelled.store(true, Ordering::Relaxed);
                SearchOutcome::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder(path: &Path) -> AuthorizedFolder {
        AuthorizedFolder {
            path: path.to_path_buf(),
            label: "Documentos".into(),
        }
    }

    fn terms(words: &[&str]) -> FileQuery {
        FileQuery {
            exact_name: None,
            terms: words.iter().map(|word| (*word).to_owned()).collect(),
        }
    }

    async fn search(query: FileQuery, root: &Path) -> SearchOutcome {
        LocalFileSearch
            .search(query, vec![folder(root)], CancellationToken::new())
            .await
    }

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("clase")).unwrap();
        fs::write(
            dir.path().join("clase/Informe Programación.docx"),
            "contenido",
        )
        .unwrap();
        fs::write(dir.path().join("proyecto.pdf"), "x").unwrap();
        fs::write(dir.path().join("recetas.txt"), "x").unwrap();
        dir
    }

    #[tokio::test]
    async fn finds_by_terms_ignoring_case_and_accents() {
        let dir = tree();
        let outcome = search(terms(&["informe", "programacion"]), dir.path()).await;
        assert_eq!(outcome.files.len(), 1);
        let file = &outcome.files[0];
        assert_eq!(file.name, "Informe Programación");
        assert_eq!(file.extension, ".docx");
        assert_eq!(file.location, "Documentos");
        assert!(file.exists);
        assert!(Path::new(&file.path).is_absolute());
    }

    #[tokio::test]
    async fn finds_an_exact_file_name() {
        let dir = tree();
        let query = FileQuery {
            exact_name: Some("proyecto.pdf".into()),
            terms: vec![],
        };
        assert_eq!(search(query, dir.path()).await.files.len(), 1);
    }

    #[tokio::test]
    async fn no_match_returns_nothing() {
        let dir = tree();
        assert!(search(terms(&["inexistente"]), dir.path())
            .await
            .files
            .is_empty());
    }

    #[tokio::test]
    async fn only_searches_inside_authorized_folders() {
        let parent = tempfile::tempdir().unwrap();
        let allowed = parent.path().join("permitida");
        let other = parent.path().join("otra");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("secreto.txt"), "x").unwrap();
        assert!(search(terms(&["secreto"]), &allowed).await.files.is_empty());
    }

    #[tokio::test]
    async fn a_missing_folder_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(search(terms(&["a"]), &dir.path().join("no-existe"))
            .await
            .files
            .is_empty());
    }

    #[tokio::test]
    async fn results_are_limited() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..(MAX_RESULTS + 5) {
            fs::write(dir.path().join(format!("informe{index}.txt")), "x").unwrap();
        }
        let outcome = search(terms(&["informe"]), dir.path()).await;
        assert_eq!(outcome.files.len(), MAX_RESULTS);
        assert!(outcome.truncated);
    }

    #[tokio::test]
    async fn skips_hidden_and_dependency_folders() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/informe.js"), "x").unwrap();
        assert!(search(terms(&["informe"]), dir.path())
            .await
            .files
            .is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_follow_symbolic_links_out_of_the_folder() {
        let outside = tree();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("enlace")).unwrap();
        assert!(search(terms(&["proyecto"]), dir.path())
            .await
            .files
            .is_empty());
    }

    #[tokio::test]
    async fn content_matches_only_appear_when_no_name_matches() {
        let dir = tree();
        fs::write(
            dir.path().join("apuntes.md"),
            "Clase 3: normalización en bases de datos",
        )
        .unwrap();
        let by_content = search(terms(&["bases", "datos"]), dir.path()).await;
        let names: Vec<_> = by_content
            .files
            .iter()
            .map(|file| (file.name.as_str(), file.matched_in_content))
            .collect();
        assert_eq!(names, [("apuntes", true)]);
        fs::write(dir.path().join("bases datos.pdf"), "x").unwrap();
        let by_name = search(terms(&["bases", "datos"]), dir.path()).await;
        let names: Vec<_> = by_name
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        assert_eq!(names, ["bases datos"]);
    }

    #[tokio::test]
    async fn the_most_specific_name_comes_first() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("prueba de rendimiento extendida.txt"), "x").unwrap();
        fs::write(dir.path().join("documento-prueba.pdf"), "x").unwrap();
        fs::write(dir.path().join("err-base.txt"), "una prueba").unwrap();
        let outcome = search(terms(&["prueba"]), dir.path()).await;
        let names: Vec<_> = outcome
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["documento-prueba", "prueba de rendimiento extendida"]
        );
    }

    #[tokio::test]
    async fn the_search_does_not_modify_files() {
        let dir = tree();
        let file = dir.path().join("proyecto.pdf");
        let before = fs::metadata(&file).unwrap().modified().unwrap();
        search(terms(&["proyecto"]), dir.path()).await;
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), before);
    }
}
