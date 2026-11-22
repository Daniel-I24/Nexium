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

mod ai;
mod files;
mod flow;
mod screen;
