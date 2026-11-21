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
