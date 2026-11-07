//! Núcleo: resuelve la intención, ejecuta su handler y registra la interacción.

use std::sync::Arc;

use chrono::Local;
use tokio_util::sync::CancellationToken;

use super::{
    handlers::{CommandHandler, HandlerContext, Outcome, ORIGIN_LOCAL},
    rules,
};
use crate::{
    error::AiError,
    models::{AssistantResponse, Category, FileQuery, Intent, InteractionStatus, ResponseBody, Stage},
    providers::{
        ai::{AiProvider, InterpretRequest, VisionRequest},
        capture::{CaptureError, ScreenCaptureProvider},
    },
    services::{history::HistoryService, secrets::SecretStore, settings::SettingsService, usage::UsageService},
};

pub const MAX_REQUEST_LENGTH: usize = 500;

pub type StageSink = Arc<dyn Fn(Stage) + Send + Sync>;

pub struct Assistant {
    pub ai: Arc<dyn AiProvider>,
    pub secrets: Arc<SecretStore>,
    pub capture: Arc<dyn ScreenCaptureProvider>,
    pub handlers: Vec<Box<dyn CommandHandler>>,
    pub settings: SettingsService,
    pub history: HistoryService,
    pub usage: UsageService,
}

enum Resolved {
    Local(Intent),
    Ai(Intent),
    /// La IA no estaba disponible, pero la búsqueda de archivos puede seguir con reglas locales.
    Degraded(Intent, String),
    Failed(Outcome),
}

impl Assistant {
    /// Comprueba lo que no cuesta nada antes de gastar cuota.
    fn check_ai_available(&self) -> Result<(), AiError> {
        if self.secrets.get(self.ai.key_account(), self.ai.dev_key_variable()).is_none() {
            return Err(AiError::NoApiKey);
        }
        let limit = self.settings.get().map_or(u32::MAX, |settings| settings.daily_ai_limit);
        let (requests, _) = self.usage.today_totals().unwrap_or((0, 0));
        if requests >= limit {
            return Err(AiError::LocalLimitReached);
        }
        Ok(())
    }

    /// Mensaje en español para cada error de IA. Ninguno se reintenta ni cambia de plan automáticamente.
    pub fn describe_ai_error(&self, error: &AiError) -> (String, Option<String>) {
        let provider = self.ai.display_name();
        let local_hint = "Las consultas de hora, sistema y red siguen funcionando.".to_owned();
        match error {
            AiError::NoApiKey => (format!("Para esto necesito una clave de {provider}."), Some(format!("Configúrala en Ajustes. {local_hint}"))),
            AiError::InvalidKey => (format!("{provider} rechazó la clave de API."), Some(format!("Revísala en Ajustes. {local_hint}"))),
            AiError::QuotaExhausted => (self.ai.quota_message(), Some(local_hint)),
            AiError::RateLimited => (format!("{provider} recibió demasiadas solicitudes en poco tiempo."), Some(format!("Espera un minuto y vuelve a intentarlo. {local_hint}"))),
            AiError::LocalLimitReached => ("Alcanzaste el límite diario de solicitudes de IA configurado en Nexium.".into(), Some(format!("Puedes cambiarlo en Ajustes. {local_hint}"))),
            AiError::ModelNotFound(model) => (format!("{provider} no reconoce el modelo \u{201c}{model}\u{201d}."), Some("Elige otro modelo en Ajustes.".into())),
            AiError::Unavailable => (format!("{provider} no está disponible en este momento."), Some(local_hint)),
            AiError::Network => (format!("No pude conectar con {provider}."), Some(format!("Comprueba tu conexión a Internet. {local_hint}"))),
            AiError::Timeout => (format!("{provider} tardó demasiado en responder."), Some(local_hint)),
            AiError::InvalidResponse => ("No pude interpretar la respuesta del modelo.".into(), Some("Prueba a escribir la solicitud de otra forma.".into())),
            AiError::Rejected(reason) => (format!("{provider} rechazó la solicitud. {reason}"), None),
            AiError::Cancelled => ("Operación cancelada.".into(), None),
        }
    }

    fn ai_failure(&self, category: Category, error: &AiError) -> Outcome {
        let (message, hint) = self.describe_ai_error(error);
        Outcome::failed(category, error.code(), message, hint)
    }

    async fn resolve(&self, text: &str, cancel: &CancellationToken, stage: &StageSink) -> Resolved {
        if let Some(intent) = rules::match_local(text) {
            return Resolved::Local(intent);
        }
        let attempt = match self.check_ai_available() {
            Err(error) => Err(error),
            Ok(()) => {
                stage(Stage::Interpreting);
                let model = self.settings.get().map(|settings| settings.interpret_model).unwrap_or_default();
                let request = InterpretRequest { text: text.to_owned(), now: Local::now(), model };
                tokio::select! {
                    answer = self.ai.interpret(request) => answer,
                    () = cancel.cancelled() => Err(AiError::Cancelled),
                }
            }
        };
        match attempt {
            Ok(answer) => {
                let _ = self.usage.record(answer.tokens);
                // La fecha que elija el modelo no cuenta: si admite dos lecturas, se pregunta.
                let doubt = matches!(answer.value, Intent::CreateReminder(_)).then(|| rules::ambiguous_date_question(text)).flatten();
                Resolved::Ai(doubt.unwrap_or(answer.value))
            }
            Err(AiError::Cancelled) => Resolved::Failed(Outcome { status: InteractionStatus::Cancelled, ..Outcome::failed(Category::General, "cancelled", "Operación cancelada.", None) }),
            Err(error) if rules::is_search_request(text) => {
                let terms = rules::local_search_terms(text);
                if terms.is_empty() {
                    return Resolved::Failed(self.ai_failure(Category::Files, &error));
                }
                let (message, _) = self.describe_ai_error(&error);
                Resolved::Degraded(Intent::SearchFiles(FileQuery { exact_name: None, terms }), format!("Búsqueda básica sin IA. {message}"))
            }
            Err(error) => Resolved::Failed(self.ai_failure(Category::General, &error)),
        }
    }

    fn finish(&self, request: &str, outcome: Outcome, origin: &str) -> AssistantResponse {
        let (id, created_at) = self.history.record(request, outcome.category, outcome.status, &outcome.summary).unwrap_or_else(|_| (0, Local::now().to_rfc3339()));
        let origin_label = origin_label(outcome.source.as_deref(), origin);
        AssistantResponse { id, request: request.to_owned(), created_at, category: outcome.category, status: outcome.status, origin_label, summary: outcome.summary, body: outcome.body }
    }

    pub async fn process(&self, text: &str, cancel: CancellationToken, stage: StageSink) -> AssistantResponse {
        let text = text.trim();
        let provider = self.ai.display_name().to_owned();
        let (intent, origin, note) = match self.resolve(text, &cancel, &stage).await {
            Resolved::Local(intent @ Intent::Clarify { .. }) => (intent, "reglas locales, sin IA".to_owned(), None),
            Resolved::Local(intent) => (intent, ORIGIN_LOCAL.to_owned(), None),
            Resolved::Ai(intent) => (intent, format!("solicitud interpretada con IA ({provider})"), None),
            Resolved::Degraded(intent, note) => (intent, "reglas locales, sin IA".to_owned(), Some(note)),
            Resolved::Failed(outcome) => return self.finish(text, outcome, "sin datos"),
        };
        stage(Stage::Executing);
        let context = HandlerContext { request: text.to_owned(), now: Local::now(), cancel: cancel.clone() };
        let Some(handler) = self.handlers.iter().find(|handler| handler.can_handle(&intent)) else {
            return self.finish(text, Outcome::failed(Category::General, "noHandler", "No sé hacer esa operación.", None), "sin datos");
        };
        let mut outcome = handler.handle(&intent, &context).await;
        if cancel.is_cancelled() {
            outcome = Outcome { status: InteractionStatus::Cancelled, ..Outcome::failed(outcome.category, "cancelled", "Operación cancelada.", None) };
        }
        if let (Some(extra), ResponseBody::Files { note: file_note, .. }) = (note, &mut outcome.body) {
            *file_note = Some(extra);
        }
        stage(Stage::Responding);
        self.finish(text, outcome, &origin)
    }

    /// Una captura por confirmación expresa del usuario. La imagen solo vive en esta función.
    pub async fn analyze_screen(&self, question: &str, cancel: CancellationToken, stage: StageSink) -> AssistantResponse {
        let question = question.trim();
        let provider = self.ai.display_name().to_owned();
        if let Err(error) = self.check_ai_available() {
            return self.finish(question, self.ai_failure(Category::Screen, &error), "sin datos");
        }
        stage(Stage::Capturing);
        let image = match self.capture.capture_once().await {
            Ok(image) => image,
            Err(CaptureError::PermissionDenied(message) | CaptureError::Failed(message)) => return self.finish(question, Outcome::failed(Category::Screen, "capture", message, None), "sin datos"),
        };
        if cancel.is_cancelled() {
            return self.finish(question, Outcome { status: InteractionStatus::Cancelled, ..Outcome::failed(Category::Screen, "cancelled", "Operación cancelada.", None) }, "sin datos");
        }
        stage(Stage::Analyzing);
        let model = self.settings.get().map(|settings| settings.vision_model).unwrap_or_default();
        let request = VisionRequest { image_jpeg: image, question: question.to_owned(), model };
        let answer = tokio::select! {
            answer = self.ai.analyze_image(request) => answer,
            () = cancel.cancelled() => Err(AiError::Cancelled),
        };
        match answer {
            Ok(answer) => {
                let _ = self.usage.record(answer.tokens);
                stage(Stage::Responding);
                let summary: String = answer.value.chars().take(160).collect();
                self.finish(question, Outcome::done(Category::Screen, summary, ResponseBody::Text { text: answer.value }), &format!("una captura analizada con IA ({provider}). Nexium ya descartó la imagen"))
            }
            Err(AiError::Cancelled) => self.finish(question, Outcome { status: InteractionStatus::Cancelled, ..Outcome::failed(Category::Screen, "cancelled", "Operación cancelada.", None) }, "sin datos"),
            Err(error) => self.finish(question, self.ai_failure(Category::Screen, &error), "sin datos"),
        }
    }
}

/// Un dato externo no es una lectura del equipo: se nombra su fuente.
fn origin_label(source: Option<&str>, origin: &str) -> String {
    match source {
        Some(source) if origin == ORIGIN_LOCAL => format!("{source}, sin IA"),
        Some(source) => format!("{source}; {origin}"),
        None => origin.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_origin_names_an_external_source() {
        assert_eq!(origin_label(Some("Open-Meteo"), ORIGIN_LOCAL), "Open-Meteo, sin IA");
        assert_eq!(origin_label(Some("Open-Meteo"), "solicitud interpretada con IA (Gemini)"), "Open-Meteo; solicitud interpretada con IA (Gemini)");
        assert_eq!(origin_label(None, ORIGIN_LOCAL), ORIGIN_LOCAL);
    }
    use crate::{
        core::handlers::{ClarifyHandler, FilesHandler, ReminderIntentHandler, ScreenHandler, SystemHandler, TimeHandler},
        database::Database,
        error::AiError,
        models::{BatteryInfo, MemoryInfo, Reading, ReminderRequest, WifiInfo},
        providers::{
            ai::{AiAnswer, AiProvider},
            capture::CaptureError,
            files::{AuthorizedFolder, LocalFileSearch},
            system::SystemProvider,
        },
        services::{reminders::ReminderService, secrets::{SecretBackend, SecretStore}},
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
    }

    #[async_trait]
    impl AiProvider for FakeAi {
        fn id(&self) -> &str {
            "fake"
        }
        fn display_name(&self) -> &str {
            "Proveedor de prueba"
        }
        fn data_notice(&self) -> &str {
            "aviso"
        }
        fn key_url(&self) -> &str {
            "x"
        }
        fn key_account(&self) -> &str {
            "fake"
        }
        fn dev_key_variable(&self) -> &str {
            "NEXIUM_TEST_NO_KEY"
        }
        fn quota_message(&self) -> String {
            "Cuota agotada de prueba".into()
        }
        async fn interpret(&self, _: InterpretRequest) -> Result<AiAnswer<Intent>, AiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.answer.lock().unwrap().take().unwrap_or(Ok(Intent::Unknown)).map(|value| AiAnswer { value, tokens: 7 })
        }
        async fn analyze_image(&self, request: VisionRequest) -> Result<AiAnswer<String>, AiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(!request.image_jpeg.is_empty());
            Ok(AiAnswer { value: "Un error de ruta".into(), tokens: 9 })
        }
    }

    struct FakeSystem;

    #[async_trait]
    impl SystemProvider for FakeSystem {
        async fn cpu_percent(&self) -> Reading<f64> {
            Reading::Value(25.0)
        }
        async fn memory(&self) -> Reading<MemoryInfo> {
            Reading::Value(MemoryInfo { used_bytes: 8 << 30, available_bytes: 8 << 30, total_bytes: 16 << 30 })
        }
        async fn battery(&self) -> Reading<BatteryInfo> {
            Reading::Unavailable("Este equipo no tiene batería.".into())
        }
        async fn internet(&self) -> Reading<bool> {
            Reading::Value(true)
        }
        async fn wifi(&self) -> Reading<WifiInfo> {
            Reading::Value(WifiInfo { connected: true, ssid: Reading::Unavailable("oculto".into()) })
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

    struct Key(bool);

    impl SecretBackend for Key {
        fn read(&self, _: &str) -> Result<Option<String>, String> {
            Ok(self.0.then(|| "k".to_owned()))
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    struct Harness {
        assistant: Assistant,
        ai_calls: Arc<AtomicUsize>,
        capture_calls: Arc<AtomicUsize>,
        _folder: tempfile::TempDir,
    }

    fn harness(answer: Option<Result<Intent, AiError>>, has_key: bool, capture_fails: bool) -> Harness {
        let db = Database::in_memory().unwrap();
        let settings = SettingsService::new(db.clone());
        let ai_calls = Arc::new(AtomicUsize::new(0));
        let capture_calls = Arc::new(AtomicUsize::new(0));
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("informe programacion.txt"), "x").unwrap();
        let handlers: Vec<Box<dyn CommandHandler>> = vec![
            Box::new(TimeHandler),
            Box::new(SystemHandler { system: Arc::new(FakeSystem) }),
            Box::new(FilesHandler {
                search: Arc::new(LocalFileSearch),
                settings: settings.clone(),
                known_folders: vec![AuthorizedFolder { path: folder.path().to_path_buf(), label: "Documentos".into() }],
            }),
            Box::new(ReminderIntentHandler { reminders: ReminderService::new(db.clone()) }),
            Box::new(ScreenHandler),
            Box::new(ClarifyHandler),
        ];
        let assistant = Assistant {
            ai: Arc::new(FakeAi { calls: ai_calls.clone(), answer: Mutex::new(answer) }),
            secrets: Arc::new(SecretStore::new(Box::new(Key(has_key)))),
            capture: Arc::new(FakeCapture { calls: capture_calls.clone(), fail: capture_fails }),
            handlers,
            settings,
            history: HistoryService::new(db.clone()),
            usage: UsageService::new(db),
        };
        Harness { assistant, ai_calls, capture_calls, _folder: folder }
    }

    fn silent() -> StageSink {
        Arc::new(|_| {})
    }

    async fn ask(assistant: &Assistant, text: &str) -> AssistantResponse {
        assistant.process(text, CancellationToken::new(), silent()).await
    }

    fn authorize(harness: &Harness) {
        let mut settings = harness.assistant.settings.get().unwrap();
        settings.authorized_folders = vec![harness._folder.path().to_string_lossy().into_owned()];
        settings.folders_confirmed = true;
        harness.assistant.settings.save(&settings).unwrap();
    }

    #[tokio::test]
    async fn local_questions_never_spend_quota() {
        let h = harness(None, true, false);
        for phrase in ["¿Qué hora es?", "¿Cuánta RAM uso?", "uso de CPU", "¿Cuánta batería tengo?", "¿Estoy conectado a Internet?", "¿A qué red Wi-Fi estoy conectado?"] {
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
        let ResponseBody::Rows { rows, note, .. } = response.body else { panic!() };
        assert!(rows[0].unavailable);
        assert_eq!(rows[0].value, "No disponible");
        assert_eq!(note.as_deref(), Some("Este equipo no tiene batería."));
    }

    #[tokio::test]
    async fn ai_intent_runs_the_controlled_handler_and_counts_usage() {
        let reminder = ReminderRequest { content: "Entregar".into(), date: NaiveDate::from_ymd_opt(2099, 1, 1), time: None, ambiguity: None };
        let h = harness(Some(Ok(Intent::CreateReminder(reminder))), true, false);
        let response = ask(&h.assistant, "Recuérdame entregar algo").await;
        assert!(matches!(response.body, ResponseBody::ReminderDraft { question: Some(_), .. }));
        assert_eq!(response.status, InteractionStatus::Pending);
        assert!(response.origin_label.contains("Proveedor de prueba"));
        assert_eq!(h.assistant.usage.today_totals().unwrap(), (1, 7));
    }

    #[tokio::test]
    async fn an_ambiguous_numeric_date_needs_neither_ai_nor_key() {
        let h = harness(None, false, false);
        let response = ask(&h.assistant, "Recuérdame pagar el 5/6").await;
        let ResponseBody::Clarification { question, suggestions } = response.body else { panic!() };
        assert_eq!(question, "¿Te refieres al 5 de junio o al 6 de mayo?");
        assert_eq!(suggestions.iter().map(|s| s.request.as_str()).collect::<Vec<_>>(), ["Recuérdame pagar el 5 de junio", "Recuérdame pagar el 6 de mayo"]);
        assert_eq!(response.origin_label, "reglas locales, sin IA");
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_model_cannot_settle_an_ambiguous_numeric_date() {
        let chosen = ReminderRequest { content: "Pagar".into(), date: NaiveDate::from_ymd_opt(2099, 6, 5), time: NaiveTime::from_hms_opt(10, 0, 0), ambiguity: None };
        let h = harness(Some(Ok(Intent::CreateReminder(chosen))), true, false);
        let response = ask(&h.assistant, "No me dejes olvidar pagar el 5/6").await;
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(response.body, ResponseBody::Clarification { ref question, .. } if question == "¿Te refieres al 5 de junio o al 6 de mayo?"));
    }

    #[tokio::test]
    async fn quota_exhausted_shows_a_clear_message_without_retrying() {
        let h = harness(Some(Err(AiError::QuotaExhausted)), true, false);
        let response = ask(&h.assistant, "cuéntame algo sobre el proyecto").await;
        let ResponseBody::Error { code, message, hint } = response.body else { panic!() };
        assert_eq!(code, "quotaExhausted");
        assert_eq!(message, "Cuota agotada de prueba");
        assert!(hint.unwrap().contains("siguen funcionando"));
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
        assert_eq!(response.status, InteractionStatus::Failed);
    }

    #[tokio::test]
    async fn without_a_key_no_call_is_made() {
        let h = harness(None, false, false);
        let response = ask(&h.assistant, "cuéntame algo").await;
        assert!(matches!(response.body, ResponseBody::Error { ref code, .. } if code == "noApiKey"));
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_daily_limit_stops_ai_calls() {
        let h = harness(None, true, false);
        let mut settings = h.assistant.settings.get().unwrap();
        settings.daily_ai_limit = 1;
        h.assistant.settings.save(&settings).unwrap();
        h.assistant.usage.record(1).unwrap();
        let response = ask(&h.assistant, "cuéntame algo").await;
        assert!(matches!(response.body, ResponseBody::Error { ref code, .. } if code == "localLimit"));
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
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
        let ResponseBody::Files { files, note, .. } = response.body else { panic!("{:?}", response.body) };
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
        let response = h.assistant.analyze_screen("Explícame este error", CancellationToken::new(), silent()).await;
        assert_eq!(h.capture_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(response.body, ResponseBody::Text { .. }));
        assert!(response.origin_label.contains("descartó la imagen"));
        let entries = h.assistant.history.list(None).unwrap();
        assert_eq!(entries[0].response, "Un error de ruta");
    }

    #[tokio::test]
    async fn without_a_key_no_capture_is_taken() {
        let h = harness(None, false, false);
        h.assistant.analyze_screen("x", CancellationToken::new(), silent()).await;
        assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_capture_permission_error_is_explained() {
        let h = harness(None, true, true);
        let response = h.assistant.analyze_screen("x", CancellationToken::new(), silent()).await;
        assert!(matches!(response.body, ResponseBody::Error { ref message, .. } if message == "sin permiso"));
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
        h.assistant.process("¿Qué hora es?", CancellationToken::new(), Arc::new(move |stage| sink.lock().unwrap().push(stage))).await;
        assert_eq!(*seen.lock().unwrap(), vec![Stage::Executing, Stage::Responding]);
    }
}
