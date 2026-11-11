//! Comprueba que Rust produce exactamente los JSON con los que se prueba la interfaz.

use nexium_lib::models::{
    AssistantResponse, Attribution, Category, DataRow, FileResult, InteractionStatus,
    ReminderOption, ResponseBody,
};
use serde_json::Value;

const CREATED: &str = "2026-11-05T14:05:00-05:00";
const LOCAL: &str = "lectura directa del equipo, sin IA";
const WITH_AI: &str = "solicitud interpretada con IA (Google Gemini)";

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../tests/contracts/responses.json")).expect("JSON válido")
}

fn response(
    id: i64,
    request: &str,
    category: Category,
    status: InteractionStatus,
    origin: &str,
    summary: &str,
    body: ResponseBody,
) -> AssistantResponse {
    AssistantResponse {
        id,
        request: request.into(),
        created_at: CREATED.into(),
        category,
        status,
        origin_label: origin.into(),
        summary: summary.into(),
        body,
    }
}

fn assert_matches(key: &str, value: &AssistantResponse) {
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        fixtures()[key],
        "contrato {key}"
    );
}

#[test]
fn reading() {
    let body = ResponseBody::Reading {
        value: "11,4".into(),
        unit: "GB".into(),
        context: "en uso de 16,0 GB".into(),
        percent: Some(71.0),
        rows: vec![DataRow::value("Disponible", "4,6 GB")],
        attribution: None,
    };
    assert_matches(
        "reading",
        &response(
            1,
            "¿Cuánta RAM uso?",
            Category::System,
            InteractionStatus::Completed,
            LOCAL,
            "11,4 GB de RAM en uso",
            body,
        ),
    );
}

#[test]
fn unavailable() {
    let body = ResponseBody::Rows {
        rows: vec![DataRow::unavailable("Batería")],
        note: Some("Este equipo no tiene batería.".into()),
        attribution: None,
    };
    assert_matches(
        "unavailable",
        &response(
            2,
            "¿Cuánta batería tengo?",
            Category::System,
            InteractionStatus::Completed,
            LOCAL,
            "Batería: no disponible",
            body,
        ),
    );
}

#[test]
fn files() {
    let file = FileResult {
        name: "Informe".into(),
        extension: ".pdf".into(),
        location: "Documentos".into(),
        path: "C:\\a\\Informe.pdf".into(),
        exists: true,
    };
    let body = ResponseBody::Files {
        summary: "1 coincidencia en Documentos".into(),
        files: vec![file],
        note: None,
    };
    assert_matches(
        "files",
        &response(
            3,
            "Busca mi informe",
            Category::Files,
            InteractionStatus::Completed,
            WITH_AI,
            "1 coincidencia en Documentos",
            body,
        ),
    );
}

#[test]
fn draft() {
    let body = ResponseBody::ReminderDraft {
        content: "Entregar".into(),
        due_at: None,
        question: Some("¿A qué hora quieres el aviso?".into()),
        options: vec![ReminderOption {
            label: "08:00".into(),
            due_at: "2026-11-12T08:00:00-05:00".into(),
        }],
    };
    assert_matches(
        "draft",
        &response(
            4,
            "Recuérdame entregar el jueves",
            Category::Reminders,
            InteractionStatus::Pending,
            WITH_AI,
            "¿A qué hora quieres el aviso?",
            body,
        ),
    );
}

#[test]
fn quota() {
    let message = "Se agotó la cuota diaria gratuita de Google Gemini.";
    let body = ResponseBody::Error {
        code: "quotaExhausted".into(),
        message: message.into(),
        hint: Some("Las consultas de hora, sistema y red siguen funcionando.".into()),
    };
    assert_matches(
        "quota",
        &response(
            5,
            "Cuéntame algo",
            Category::General,
            InteractionStatus::Failed,
            "sin datos",
            message,
            body,
        ),
    );
}

#[test]
fn capture_request() {
    let body = ResponseBody::CaptureRequest {
        question: "Explícame este error".into(),
    };
    assert_matches(
        "capture",
        &response(
            6,
            "Explícame este error",
            Category::Screen,
            InteractionStatus::Pending,
            LOCAL,
            "Pendiente de tu permiso para capturar",
            body,
        ),
    );
}

#[test]
fn weather() {
    let body = ResponseBody::Reading {
        value: "15".into(),
        unit: "°C".into(),
        context: "nublado, ahora en Bogotá, Colombia".into(),
        percent: None,
        rows: vec![DataRow::value("Pronóstico de hoy", "9 °C a 19 °C · lluvia")],
        attribution: Some(Attribution {
            text: "Datos meteorológicos de Open-Meteo (CC BY 4.0)".into(),
            url: "https://open-meteo.com/".into(),
        }),
    };
    assert_matches(
        "weather",
        &response(
            7,
            "¿Cómo está el clima?",
            Category::Weather,
            InteractionStatus::Completed,
            "Open-Meteo, sin IA",
            "15 °C",
            body,
        ),
    );
}
