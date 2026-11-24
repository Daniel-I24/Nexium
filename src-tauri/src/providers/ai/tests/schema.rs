use super::*;
use serde_json::json;

#[test]
fn html_entities_and_control_characters_are_cleaned() {
    assert_eq!(
        decode_entities("\u{19}A qu&#xe9; hora de la tarde deseas el aviso?"),
        "A qué hora de la tarde deseas el aviso?"
    );
    assert_eq!(
        decode_entities("Tom &amp; Jerry &#191;s&#237;?"),
        "Tom & Jerry ¿sí?"
    );
    assert_eq!(decode_entities("dos\nlíneas"), "dos\nlíneas");
}

#[test]
fn a_document_summary_keeps_its_ideas_and_data_and_rejects_too_many() {
    let summary = parse_document_summary(
        r#"{"summary":"Guía de **Bases de datos**. Tiene tres unidades.","points":["Normalización: evita datos repetidos."],"details":["Parcial 1: 30 %."]}"#,
    )
    .unwrap();
    assert_eq!(
        summary.summary,
        "Guía de Bases de datos. Tiene tres unidades."
    );
    assert_eq!(summary.points, ["Normalización: evita datos repetidos."]);
    assert_eq!(summary.details, ["Parcial 1: 30 %."]);
    let nine = serde_json::json!({ "summary": "s", "points": vec!["p"; 9], "details": [] });
    assert!(parse_document_summary(&nine.to_string()).is_err());
    // Una respuesta anterior, sin datos importantes, sigue siendo válida.
    assert!(parse_document_summary(r#"{"summary":"s","points":[]}"#).is_ok());
}

fn parse(text: &str) -> Result<Intent, AiError> {
    parse_interpretation(text, "pregunta").map(|mut actions| actions.remove(0).intent)
}

#[test]
fn extracts_a_complete_reminder() {
    let intent = parse(r#"{"intent":"create_reminder","reminder_content":"entregar el proyecto","reminder_date":"2026-11-12","reminder_time":"08:00"}"#).unwrap();
    let Intent::CreateReminder(reminder) = intent else {
        panic!()
    };
    assert_eq!(reminder.content, "entregar el proyecto");
    assert_eq!(reminder.date, NaiveDate::from_ymd_opt(2026, 11, 12));
    assert_eq!(reminder.time, NaiveTime::from_hms_opt(8, 0, 0));
    assert_eq!(reminder.ambiguity, None);
}

#[test]
fn an_unreadable_date_becomes_a_question() {
    let intent =
        parse(r#"{"intent":"create_reminder","reminder_content":"x","reminder_date":"el jueves"}"#)
            .unwrap();
    let Intent::CreateReminder(reminder) = intent else {
        panic!()
    };
    assert_eq!(reminder.date, None);
    assert!(reminder.ambiguity.is_some());
}

#[test]
fn a_doubt_that_is_not_a_question_becomes_one() {
    let intent = parse(r#"{"intent":"create_reminder","reminder_content":"estudiar","reminder_ambiguity":"pronto"}"#).unwrap();
    let Intent::CreateReminder(reminder) = intent else {
        panic!()
    };
    assert_eq!(
        reminder.ambiguity.as_deref(),
        Some("¿Puedes indicarme la fecha y la hora exactas?")
    );
}

#[test]
fn markdown_never_reaches_the_interface() {
    let cases = [
        (
            "**Error 0x80070002**: falta `configuracion.ini`",
            "Error 0x80070002: falta configuracion.ini",
        ),
        ("Código de error: 0x80070002", "Código de error: 0x80070002"),
        (
            r"Abre `C:\Program Files\Inventario\configuracion.ini`",
            r"Abre C:\Program Files\Inventario\configuracion.ini",
        ),
        (
            "Busca `*.log` en `__pycache__`",
            "Busca *.log en __pycache__",
        ),
        (
            "El archivo __init__.py no existe",
            "El archivo __init__.py no existe",
        ),
        (
            "El archivo mi_informe_final.docx",
            "El archivo mi_informe_final.docx",
        ),
        (
            "Borra los archivos *.tmp y *.log",
            "Borra los archivos *.tmp y *.log",
        ),
        ("Calcula 2 * 3 * 4", "Calcula 2 * 3 * 4"),
        (
            "*Nota:* reinicia el *servicio*",
            "Nota: reinicia el servicio",
        ),
        ("### ¿Qué significa?", "¿Qué significa?"),
        (
            "1. Ejecuta `sfc /scannow` como administrador",
            "Ejecuta sfc /scannow como administrador",
        ),
        ("- Reinstala el programa", "Reinstala el programa"),
        ("Mira [la ayuda](https://ejemplo.test)", "Mira la ayuda"),
        (
            r"Ruta `C:\Users\[nombre]\(copia)`",
            r"Ruta C:\Users\[nombre]\(copia)",
        ),
        ("**Error sin cerrar", "Error sin cerrar"),
        ("Comilla `sin cerrar", "Comilla sin cerrar"),
        ("> cita\n\n**aviso**", "cita aviso"),
        ("Texto   con\nsaltos", "Texto con saltos"),
    ];
    for (input, expected) in cases {
        assert_eq!(plain_text(input), expected, "{input}");
    }
}

#[test]
fn interpretation_texts_are_shown_without_markdown() {
    let intent = parse(r####"{"intent":"clarify","clarification_question":"**¿Qué** quieres abrir?","suggestions":["- Buscar un archivo"]}"####).unwrap();
    assert_eq!(
        intent,
        Intent::Clarify {
            question: "¿Qué quieres abrir?".into(),
            suggestions: vec!["Buscar un archivo".into()]
        }
    );
    let Intent::CreateReminder(reminder) = parse(r####"{"intent":"create_reminder","reminder_content":"**pagar** el arriendo","reminder_ambiguity":"### ¿A qué hora?"}"####).unwrap() else { panic!() };
    assert_eq!(reminder.content, "pagar el arriendo");
    assert_eq!(reminder.ambiguity.as_deref(), Some("¿A qué hora?"));
}

#[test]
fn a_screen_analysis_has_a_summary_a_cause_and_up_to_three_steps() {
    let analysis = parse_screen_analysis(r####"{"summary":"**Falta** configuracion.ini.","cause":"El programa no encuentra su archivo.","steps":["1. Comprueba la carpeta.","- Reinstala el programa."]}"####).unwrap();
    assert_eq!(analysis.summary, "Falta configuracion.ini.");
    assert_eq!(
        analysis.cause.as_deref(),
        Some("El programa no encuentra su archivo.")
    );
    assert_eq!(
        analysis.steps,
        ["Comprueba la carpeta.", "Reinstala el programa."]
    );
}

#[test]
fn a_screen_without_problems_has_no_cause_or_steps() {
    let analysis = parse_screen_analysis(
        r#"{"summary":"Un escritorio sin ventanas abiertas.","cause":null,"steps":[]}"#,
    )
    .unwrap();
    assert_eq!(analysis.cause, None);
    assert!(analysis.steps.is_empty());
}

#[test]
fn an_invalid_screen_analysis_is_rejected() {
    for text in [
        "Un texto libre con **Markdown**",
        r#"{"summary":"","cause":null,"steps":[]}"#,
        r#"{"summary":"x","steps":[]}"#,
        r#"{"summary":"x","cause":null,"steps":["a","b","c","d"]}"#,
        r#"{"summary":"x","cause":null,"steps":"uno"}"#,
    ] {
        assert_eq!(
            parse_screen_analysis(text),
            Err(AiError::InvalidResponse),
            "{text}"
        );
    }
}

#[test]
fn long_texts_are_cut_between_words() {
    let long = format!(
        r#"{{"summary":"{}","cause":null,"steps":[]}}"#,
        "palabra ".repeat(80)
    );
    let summary = parse_screen_analysis(&long).unwrap().summary;
    assert!(summary.chars().count() <= MAX_TEXT + 1);
    assert!(summary.ends_with("palabra…"));
}

#[test]
fn rejects_unknown_intents_and_invalid_json() {
    assert_eq!(
        parse(r#"{"intent":"run_shell_command"}"#),
        Err(AiError::InvalidResponse)
    );
    assert_eq!(parse("no es json"), Err(AiError::InvalidResponse));
    assert_eq!(
        parse(r#"{"intent":"system","metric":"delete_all"}"#),
        Err(AiError::InvalidResponse)
    );
}

#[test]
fn a_file_name_cannot_carry_a_path() {
    assert_eq!(
        parse(r#"{"intent":"search_files","exact_file_name":"..\\..\\Windows\\system32"}"#),
        Err(AiError::InvalidResponse)
    );
    let intent = parse(
        r#"{"intent":"search_files","exact_file_name":"C:\\a.pdf","search_terms":["informe"]}"#,
    )
    .unwrap();
    assert_eq!(
        intent,
        Intent::SearchFiles(FileQuery {
            exact_name: None,
            terms: vec!["informe".into()]
        })
    );
}

#[test]
fn limits_the_number_of_terms() {
    let terms: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
    let text = json!({ "intent": "search_files", "search_terms": terms }).to_string();
    let Intent::SearchFiles(query) = parse(&text).unwrap() else {
        panic!()
    };
    assert_eq!(query.terms.len(), MAX_TERMS);
}
