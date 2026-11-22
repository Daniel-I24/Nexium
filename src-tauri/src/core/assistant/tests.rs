use super::*;
use crate::models::ScreenAnalysis;
use crate::services::performance::Timings;

#[test]
fn the_origin_names_an_external_source() {
    assert_eq!(
        origin_label(Some("Open-Meteo"), ORIGIN_LOCAL),
        "Open-Meteo, sin IA"
    );
    assert_eq!(
        origin_label(Some("Open-Meteo"), "solicitud interpretada con IA (Gemini)"),
        "Open-Meteo; solicitud interpretada con IA (Gemini)"
    );
    assert_eq!(origin_label(None, ORIGIN_LOCAL), ORIGIN_LOCAL);
}
use crate::{
    core::handlers::{
        ClarifyHandler, FilesHandler, ReminderIntentHandler, ScreenHandler, SystemHandler,
        TimeHandler,
    },
    database::Database,
    error::AiError,
    models::{BatteryInfo, MemoryInfo, Reading, ReminderRequest, WifiInfo},
    providers::{
        ai::{AiAnswer, AiProvider, AiServiceStatus},
        capture::CaptureError,
        files::{AuthorizedFolder, LocalFileSearch},
        system::SystemProvider,
    },
    services::reminders::ReminderService,
};
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveTime};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

struct FakeAi {
    calls: Arc<AtomicUsize>,
    answer: Mutex<Option<Result<Intent, AiError>>>,
    configured: bool,
}

#[async_trait]
impl AiProvider for FakeAi {
    fn display_name(&self) -> &str {
        "Proveedor de prueba"
    }
    fn data_notice(&self) -> &str {
        "aviso"
    }
    fn is_configured(&self) -> bool {
        self.configured
    }
    fn quota_message(&self) -> String {
        "Cuota agotada de prueba".into()
    }
    async fn interpret(&self, _: InterpretRequest) -> Result<AiAnswer<Intent>, AiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.answer
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(Intent::Unknown))
            .map(|value| AiAnswer { value, tokens: 7 })
    }
    async fn analyze_image(
        &self,
        request: VisionRequest,
    ) -> Result<AiAnswer<ScreenAnalysis>, AiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!request.image_jpeg.is_empty());
        if request.question == "respuesta ilegible" {
            return Err(AiError::InvalidResponse);
        }
        Ok(AiAnswer {
            value: ScreenAnalysis {
                summary: "Un error de ruta".into(),
                cause: Some("Falta un archivo.".into()),
                steps: vec!["Comprueba la carpeta.".into()],
            },
            tokens: 9,
        })
    }
    async fn analyze_document(
        &self,
        request: crate::providers::ai::DocumentRequest,
    ) -> Result<AiAnswer<crate::models::DocumentSummary>, AiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!request.text.is_empty());
        Ok(AiAnswer {
            value: crate::models::DocumentSummary {
                summary: "Trata de bases de datos.".into(),
                points: vec!["Explica la normalización.".into()],
                details: vec![],
            },
            tokens: 11,
        })
    }
}

struct FakeSystem;

#[async_trait]
impl SystemProvider for FakeSystem {
    async fn cpu_percent(&self) -> Reading<f64> {
        Reading::Value(25.0)
    }
    async fn memory(&self) -> Reading<MemoryInfo> {
        Reading::Value(MemoryInfo {
            used_bytes: 8 << 30,
            available_bytes: 8 << 30,
            total_bytes: 16 << 30,
        })
    }
    async fn battery(&self) -> Reading<BatteryInfo> {
        Reading::Unavailable("Este equipo no tiene batería.".into())
    }
    async fn internet(&self) -> Reading<bool> {
        Reading::Value(true)
    }
    async fn wifi(&self) -> Reading<WifiInfo> {
        Reading::Value(WifiInfo {
            connected: true,
            ssid: Reading::Unavailable("oculto".into()),
        })
    }
    async fn dark_mode(&self) -> Reading<bool> {
        Reading::Value(false)
    }
}

struct FakeCapture {
    calls: Arc<AtomicUsize>,
    fail: bool,
}

#[async_trait]
impl ScreenCaptureProvider for FakeCapture {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(CaptureError::PermissionDenied("sin permiso".into()))
        } else {
            Ok(vec![0xFF, 0xD8])
        }
    }
}

struct Harness {
    assistant: Assistant,
    ai_calls: Arc<AtomicUsize>,
    capture_calls: Arc<AtomicUsize>,
    _folder: tempfile::TempDir,
}

fn harness(
    answer: Option<Result<Intent, AiError>>,
    configured: bool,
    capture_fails: bool,
) -> Harness {
    let db = Database::in_memory().unwrap();
    let settings = SettingsService::new(db.clone());
    let ai_calls = Arc::new(AtomicUsize::new(0));
    let capture_calls = Arc::new(AtomicUsize::new(0));
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("informe programacion.txt"), "x").unwrap();
    let handlers: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(TimeHandler),
        Box::new(SystemHandler {
            system: Arc::new(FakeSystem),
        }),
        Box::new(FilesHandler {
            search: Arc::new(LocalFileSearch),
            settings: settings.clone(),
            known_folders: vec![AuthorizedFolder {
                path: folder.path().to_path_buf(),
                label: "Documentos".into(),
            }],
        }),
        Box::new(crate::core::handlers::DocumentsHandler {
            files: FilesHandler {
                search: Arc::new(LocalFileSearch),
                settings: settings.clone(),
                known_folders: vec![],
            },
            offers: Arc::default(),
            provider: "Proveedor de prueba".into(),
        }),
        Box::new(ReminderIntentHandler {
            reminders: ReminderService::new(db.clone()),
        }),
        Box::new(ScreenHandler),
        Box::new(ClarifyHandler),
    ];
    let assistant = Assistant {
        ai: Arc::new(FakeAi {
            calls: ai_calls.clone(),
            answer: Mutex::new(answer),
            configured,
        }),
        ai_status: AiStatusTracker::default(),
        capture: Arc::new(FakeCapture {
            calls: capture_calls.clone(),
            fail: capture_fails,
        }),
        handlers,
        settings,
        history: HistoryService::new(db.clone()),
        usage: UsageService::new(db),
        performance: Default::default(),
    };
    Harness {
        assistant,
        ai_calls,
        capture_calls,
        _folder: folder,
    }
}

fn silent() -> StageSink {
    Arc::new(|_| {})
}

async fn ask(assistant: &Assistant, text: &str) -> AssistantResponse {
    assistant
        .process(text, CancellationToken::new(), silent())
        .await
}

fn authorize(harness: &Harness) {
    let mut settings = harness.assistant.settings.get().unwrap();
    settings.authorized_folders = vec![harness._folder.path().to_string_lossy().into_owned()];
    harness.assistant.settings.save(&settings).unwrap();
}

#[tokio::test]
async fn a_request_keeps_a_single_history_entry_until_its_final_state() {
    let h = harness(None, true, false);
    let pending = ask(&h.assistant, "Resume mi informe de programación").await;
    assert_eq!(pending.status, InteractionStatus::Pending);
    authorize(&h);
    // Al permitir las carpetas, la misma solicitud sigue en su entrada.
    let offered = h
        .assistant
        .process_timed(
            "Resume mi informe de programación",
            Some(pending.id),
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert_eq!(offered.id, pending.id);
    let ResponseBody::DocumentConsent { .. } = offered.body else {
        panic!("{:?}", offered.body)
    };
    let file = h._folder.path().join("informe programacion.txt");
    std::fs::write(&file, "Informe con texto suficiente para resumirlo.").unwrap();
    let done = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Summarize,
            Some(pending.id),
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(done.id, pending.id);
    assert_eq!(done.request, "Resume mi informe de programación");
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, InteractionStatus::Completed);
    assert_eq!(history[0].response, "Trata de bases de datos.");
}

#[tokio::test]
async fn a_continuation_of_a_deleted_entry_creates_a_new_one() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .process_timed(
            "¿Qué hora es?",
            Some(999),
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert_ne!(response.id, 999);
    assert_eq!(h.assistant.history.list(None).unwrap().len(), 1);
}

#[tokio::test]
async fn a_document_needs_folders_and_then_is_offered_without_sending_anything() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "Resume mi informe de programación").await;
    assert!(matches!(response.body, ResponseBody::FolderConsent { .. }));
    authorize(&h);
    let response = ask(&h.assistant, "Resume mi informe de programación").await;
    let ResponseBody::DocumentConsent {
        candidates,
        provider,
        task,
        ..
    } = response.body
    else {
        panic!("{:?}", response.body)
    };
    assert_eq!(task, crate::models::DocumentTask::Summarize);
    assert_eq!(provider, "Proveedor de prueba");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "informe programacion");
    // Ofrecer un archivo no lo lee ni lo envía.
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_document_summary_never_stores_its_content() {
    let h = harness(None, true, false);
    let file = h._folder.path().join("acta.txt");
    std::fs::write(
        &file,
        "Contenido privado del acta de la reunión de presupuesto.",
    )
    .unwrap();
    let response = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Summarize,
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(response.request, "Resumen de «acta.txt»");
    let ResponseBody::DocumentAnalysis {
        summary, points, ..
    } = &response.body
    else {
        panic!("{:?}", response.body)
    };
    assert_eq!(summary, "Trata de bases de datos.");
    assert_eq!(points.len(), 1);
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history[0].request, "Resumen de «acta.txt»");
    assert!(!history[0].response.contains("Contenido privado"));
}

#[tokio::test]
async fn an_unreadable_document_is_explained_without_calling_the_ai() {
    let h = harness(None, true, false);
    let file = h._folder.path().join("foto.png");
    std::fs::write(&file, "x").unwrap();
    let response = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Analyze,
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(response.status, InteractionStatus::Failed);
    assert!(response.summary.contains("No sé leer archivos .png"));
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn local_questions_never_spend_quota() {
    let h = harness(None, true, false);
    for phrase in [
        "¿Qué hora es?",
        "¿Cuánta RAM uso?",
        "uso de CPU",
        "¿Cuánta batería tengo?",
        "¿Estoy conectado a Internet?",
        "¿A qué red Wi-Fi estoy conectado?",
    ] {
        let response = ask(&h.assistant, phrase).await;
        assert_eq!(response.origin_label, ORIGIN_LOCAL, "{phrase}");
    }
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.assistant.usage.today_totals().unwrap(), (0, 0));
}

#[tokio::test]
async fn every_interaction_is_recorded() {
    let h = harness(None, true, false);
    ask(&h.assistant, "¿Qué hora es?").await;
    let entries = h.assistant.history.list(None).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, Category::Time);
    assert_eq!(entries[0].status, InteractionStatus::Completed);
}

#[tokio::test]
async fn unavailable_data_is_not_invented() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "¿Cuánta batería tengo?").await;
    let ResponseBody::Rows { rows, note, .. } = response.body else {
        panic!()
    };
    assert!(rows[0].unavailable);
    assert_eq!(rows[0].value, "No disponible");
    assert_eq!(note.as_deref(), Some("Este equipo no tiene batería."));
}

#[tokio::test]
async fn ai_intent_runs_the_controlled_handler_and_counts_usage() {
    let reminder = ReminderRequest {
        content: "Entregar".into(),
        date: NaiveDate::from_ymd_opt(2099, 1, 1),
        time: None,
        ambiguity: None,
    };
    let h = harness(Some(Ok(Intent::CreateReminder(reminder))), true, false);
    let response = ask(&h.assistant, "Recuérdame entregar algo").await;
    assert!(matches!(
        response.body,
        ResponseBody::ReminderDraft {
            question: Some(_),
            ..
        }
    ));
    assert_eq!(response.status, InteractionStatus::Pending);
    assert!(response.origin_label.contains("Proveedor de prueba"));
    assert_eq!(h.assistant.usage.today_totals().unwrap(), (1, 7));
}

#[tokio::test]
async fn an_ambiguous_numeric_date_needs_neither_ai_nor_key() {
    let h = harness(None, false, false);
    let response = ask(&h.assistant, "Recuérdame pagar el 5/6").await;
    let ResponseBody::Clarification {
        question,
        suggestions,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(question, "¿Te refieres al 5 de junio o al 6 de mayo?");
    assert_eq!(
        suggestions
            .iter()
            .map(|s| s.request.as_str())
            .collect::<Vec<_>>(),
        [
            "Recuérdame pagar el 5 de junio",
            "Recuérdame pagar el 6 de mayo"
        ]
    );
    assert_eq!(response.origin_label, "reglas locales, sin IA");
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_model_cannot_settle_an_ambiguous_numeric_date() {
    let chosen = ReminderRequest {
        content: "Pagar".into(),
        date: NaiveDate::from_ymd_opt(2099, 6, 5),
        time: NaiveTime::from_hms_opt(10, 0, 0),
        ambiguity: None,
    };
    let h = harness(Some(Ok(Intent::CreateReminder(chosen))), true, false);
    let response = ask(&h.assistant, "No me dejes olvidar pagar el 5/6").await;
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
    assert!(
        matches!(response.body, ResponseBody::Clarification { ref question, .. } if question == "¿Te refieres al 5 de junio o al 6 de mayo?")
    );
}

#[tokio::test]
async fn a_region_without_service_says_so_and_keeps_local_features() {
    let h = harness(Some(Err(AiError::RegionUnavailable)), true, false);
    let response = ask(&h.assistant, "cuéntame algo sobre el proyecto").await;
    let ResponseBody::Error {
        code,
        message,
        hint,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(code, "regionUnavailable");
    assert!(message.contains("no está disponible en tu región"));
    let hint = hint.unwrap();
    assert!(hint.contains("Espacio Económico Europeo") && hint.contains("siguen funcionando"));
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_unknown_country_is_explained_apart_from_a_blocked_region() {
    let h = harness(Some(Err(AiError::RegionUnknown)), true, false);
    let response = ask(&h.assistant, "cuéntame algo sobre el proyecto").await;
    let ResponseBody::Error {
        code,
        message,
        hint,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(code, "regionUnknown");
    assert!(message.contains("No se pudo determinar tu país"));
    assert!(hint.unwrap().contains("Tor"));
}

#[test]
fn each_cause_of_failure_has_its_own_message() {
    let h = harness(None, true, false);
    let message = |error: AiError| h.assistant.describe_ai_error(&error).0;
    let messages = [
        message(AiError::Network),
        message(AiError::NotConfigured),
        message(AiError::Unavailable),
        message(AiError::Timeout),
        message(AiError::QuotaExhausted(None)),
        message(AiError::RegionUnavailable),
        message(AiError::RegionUnknown),
    ];
    assert!(messages[0].contains("Sin conexión"));
    assert!(messages[1].contains("no incluye el servicio de IA"));
    assert!(messages[2].contains("no responde"));
    assert!(messages[3].contains("tardó demasiado"));
    assert!(messages[4].contains("Cuota agotada"));
    assert!(messages[5].contains("tu región"));
    assert!(messages[6].contains("tu país"));
    let unique: std::collections::HashSet<_> = messages.iter().collect();
    assert_eq!(unique.len(), messages.len());
    assert!(messages
        .iter()
        .all(|message| !message.contains("no está disponible en este momento")));
    assert!(h
        .assistant
        .describe_ai_error(&AiError::Unavailable)
        .1
        .unwrap()
        .contains("no en tu equipo ni en tu conexión"));
}

#[tokio::test]
async fn quota_exhausted_shows_a_clear_message_without_retrying() {
    let h = harness(Some(Err(AiError::QuotaExhausted(None))), true, false);
    let response = ask(&h.assistant, "cuéntame algo sobre el proyecto").await;
    let ResponseBody::Error {
        code,
        message,
        hint,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(code, "quotaExhausted");
    assert_eq!(message, "Cuota agotada de prueba");
    assert!(hint.unwrap().contains("siguen funcionando"));
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
    assert_eq!(response.status, InteractionStatus::Failed);
}

#[tokio::test]
async fn without_the_service_no_call_is_made() {
    let h = harness(None, false, false);
    let response = ask(&h.assistant, "cuéntame algo").await;
    assert!(
        matches!(response.body, ResponseBody::Error { ref code, .. } if code == "notConfigured")
    );
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_service_status_comes_from_real_use() {
    let h = harness(Some(Err(AiError::QuotaExhausted(None))), true, false);
    assert_eq!(
        h.assistant.ai_status.current(true),
        AiServiceStatus::NotUsedYet
    );
    ask(&h.assistant, "¿Qué hora es?").await;
    assert_eq!(
        h.assistant.ai_status.current(true),
        AiServiceStatus::NotUsedYet
    );
    ask(&h.assistant, "cuéntame algo").await;
    assert_eq!(
        h.assistant.ai_status.current(true),
        AiServiceStatus::QuotaExhausted
    );
    h.assistant
        .analyze_screen("x", None, CancellationToken::new(), silent())
        .await;
    assert_eq!(
        h.assistant.ai_status.current(true),
        AiServiceStatus::Working
    );
}

#[tokio::test]
async fn a_daily_limit_says_when_it_resets() {
    let h = harness(Some(Err(AiError::DailyLimit(Some(3600)))), true, false);
    let response = ask(&h.assistant, "cuéntame algo").await;
    let ResponseBody::Error { code, hint, .. } = response.body else {
        panic!()
    };
    assert_eq!(code, "dailyLimit");
    assert!(hint.unwrap().contains("Se restablece"));
}

#[test]
fn the_reset_time_is_given_in_local_time() {
    use chrono::TimeZone;
    let now = Local.with_ymd_and_hms(2026, 11, 22, 18, 30, 0).unwrap();
    assert_eq!(
        reset_sentence(Some(1800), now),
        "Se restablece hoy a las 19:00, hora de tu equipo."
    );
    assert_eq!(
        reset_sentence(Some(6 * 3600), now),
        "Se restablece mañana a las 00:30, hora de tu equipo."
    );
    assert_eq!(reset_sentence(None, now), "Vuelve a intentarlo mañana.");
}

#[tokio::test]
async fn file_search_asks_for_authorization_first() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "¿Existe el archivo proyecto.pdf?").await;
    assert!(matches!(response.body, ResponseBody::FolderConsent { .. }));
}

#[tokio::test]
async fn search_falls_back_to_local_terms_when_ai_is_unavailable() {
    let h = harness(None, false, false);
    authorize(&h);
    let response = ask(&h.assistant, "Busca mi informe de programación").await;
    let ResponseBody::Files { files, note, .. } = response.body else {
        panic!("{:?}", response.body)
    };
    assert_eq!(files.len(), 1);
    assert!(note.unwrap().contains("sin IA"));
    assert_eq!(response.origin_label, "reglas locales, sin IA");
}

#[tokio::test]
async fn a_screen_request_only_asks_for_permission() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "Explícame este error").await;
    assert!(matches!(response.body, ResponseBody::CaptureRequest { .. }));
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn analysis_captures_once_and_does_not_keep_the_image() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .analyze_screen(
            "Explícame este error",
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        response.body,
        ResponseBody::ScreenAnalysis {
            summary: "Un error de ruta".into(),
            cause: Some("Falta un archivo.".into()),
            steps: vec!["Comprueba la carpeta.".into()]
        }
    );
    assert!(response.origin_label.contains("descartó la imagen"));
    let entries = h.assistant.history.list(None).unwrap();
    assert_eq!(entries[0].response, "Un error de ruta");
}

#[tokio::test]
async fn an_unreadable_analysis_shows_a_clear_message() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .analyze_screen(
            "respuesta ilegible",
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    let ResponseBody::Error {
        code,
        message,
        hint,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(code, "invalidResponse");
    assert!(message.contains("formato que Nexium no puede mostrar"));
    assert!(hint.unwrap().contains("ya se descartó"));
}

#[tokio::test]
async fn without_the_service_no_capture_is_taken() {
    let h = harness(None, false, false);
    h.assistant
        .analyze_screen("x", None, CancellationToken::new(), silent())
        .await;
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_capture_permission_error_is_explained() {
    let h = harness(None, true, true);
    let response = h
        .assistant
        .analyze_screen("x", None, CancellationToken::new(), silent())
        .await;
    assert!(
        matches!(response.body, ResponseBody::Error { ref message, .. } if message == "sin permiso")
    );
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_cancelled_request_is_recorded_as_cancelled() {
    let h = harness(None, true, false);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let response = h.assistant.process("cuéntame algo", cancel, silent()).await;
    assert_eq!(response.status, InteractionStatus::Cancelled);
}

#[tokio::test]
async fn stages_are_reported_in_order() {
    let h = harness(None, true, false);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    h.assistant
        .process(
            "¿Qué hora es?",
            CancellationToken::new(),
            Arc::new(move |stage| sink.lock().unwrap().push(stage)),
        )
        .await;
    assert_eq!(
        *seen.lock().unwrap(),
        vec![Stage::Executing, Stage::Responding]
    );
}
