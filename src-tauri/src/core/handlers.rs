//! Un `CommandHandler` por operación. El enrutador solo conoce esta interfaz.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};
use futures_util::join;
use tokio_util::sync::CancellationToken;

use super::format::{clock, decimal, gigabytes, long_date, whole};
use crate::{
    error::AppError,
    models::{Attribution, Category, DataRow, FileQuery, Intent, InteractionStatus, Reading, ReminderOption, ReminderRequest, ResponseBody, SuggestedRequest, SystemMetric, WeatherWhen},
    providers::{
        files::{AuthorizedFolder, FileSearchProvider},
        system::SystemProvider,
        weather::{WeatherError, WeatherProvider, ATTRIBUTION_TEXT, ATTRIBUTION_URL},
    },
    services::{reminders::ReminderService, settings::SettingsService},
};

pub const ORIGIN_LOCAL: &str = "lectura directa del equipo, sin IA";

#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub category: Category,
    pub status: InteractionStatus,
    pub summary: String,
    pub body: ResponseBody,
    /// Origen de los datos, si no es el equipo. La IA interpreta, pero los datos no salen de ella.
    pub source: Option<String>,
}

impl Outcome {
    pub fn done(category: Category, summary: impl Into<String>, body: ResponseBody) -> Self {
        Self { category, status: InteractionStatus::Completed, summary: summary.into(), body, source: None }
    }

    pub fn failed(category: Category, code: &str, message: impl Into<String>, hint: Option<String>) -> Self {
        let message = message.into();
        Self {
            category,
            status: InteractionStatus::Failed,
            summary: message.clone(),
            body: ResponseBody::Error { code: code.into(), message, hint },
            source: None,
        }
    }

    pub fn pending(category: Category, summary: impl Into<String>, body: ResponseBody) -> Self {
        Self { category, status: InteractionStatus::Pending, summary: summary.into(), body, source: None }
    }
}

pub struct HandlerContext {
    pub request: String,
    pub now: DateTime<Local>,
    pub cancel: CancellationToken,
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    fn can_handle(&self, intent: &Intent) -> bool;
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Outcome;
}

pub struct TimeHandler;

#[async_trait]
impl CommandHandler for TimeHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::Time)
    }

    async fn handle(&self, _: &Intent, context: &HandlerContext) -> Outcome {
        let time = clock(&context.now);
        let date = long_date(context.now.date_naive());
        Outcome::done(
            Category::Time,
            format!("{time} · {date}"),
            ResponseBody::Reading { value: time, unit: String::new(), context: date, percent: None, rows: vec![], attribution: None },
        )
    }
}

pub struct SystemHandler {
    pub system: Arc<dyn SystemProvider>,
}

fn unavailable_outcome(label: &str, reason: String) -> Outcome {
    Outcome::done(
        Category::System,
        format!("{label}: no disponible"),
        ResponseBody::Rows { rows: vec![DataRow::unavailable(label)], note: Some(reason), attribution: None },
    )
}

fn row_for<T>(label: &str, reading: Reading<T>, format: impl Fn(T) -> String) -> DataRow {
    match reading {
        Reading::Value(value) => DataRow::value(label, format(value)),
        Reading::Unavailable(_) => DataRow::unavailable(label),
    }
}

fn yes_no(value: bool, yes: &str, no: &str) -> String {
    (if value { yes } else { no }).to_owned()
}

impl SystemHandler {
    async fn wifi(&self) -> Outcome {
        match self.system.wifi().await {
            Reading::Value(info) => {
                let mut rows = vec![DataRow::value("Estado de Wi-Fi", yes_no(info.connected, "Conectado", "Sin conexión"))];
                let mut note = None;
                let mut summary = yes_no(info.connected, "Wi-Fi conectado", "Wi-Fi sin conexión");
                if info.connected {
                    match info.ssid {
                        Reading::Value(name) => {
                            summary = format!("Conectado a {name}");
                            rows.push(DataRow::value("Nombre de la red", name));
                        }
                        Reading::Unavailable(reason) => {
                            rows.push(DataRow::unavailable("Nombre de la red"));
                            note = Some(reason);
                        }
                    }
                }
                Outcome::done(Category::System, summary, ResponseBody::Rows { rows, note, attribution: None })
            }
            Reading::Unavailable(reason) => unavailable_outcome("Estado de Wi-Fi", reason),
        }
    }

    async fn overview(&self) -> Outcome {
        let (cpu, memory, battery, internet, wifi, dark) = join!(self.system.cpu_percent(), self.system.memory(), self.system.battery(), self.system.internet(), self.system.wifi(), self.system.dark_mode());
        let rows = vec![
            row_for("CPU en uso", cpu, |value| format!("{} %", whole(value))),
            row_for("RAM en uso", memory.clone(), |value| format!("{} GB de {} GB", gigabytes(value.used_bytes), gigabytes(value.total_bytes))),
            row_for("RAM disponible", memory, |value| format!("{} GB", gigabytes(value.available_bytes))),
            row_for("Batería", battery, |value| format!("{} % · {}", whole(value.percent), value.state)),
            row_for("Internet", internet, |value| yes_no(value, "Conectado", "Sin conexión")),
            row_for("Wi-Fi", wifi, |value| yes_no(value.connected, "Conectado", "Sin conexión")),
            row_for("Modo oscuro", dark, |value| yes_no(value, "Activado", "Desactivado")),
        ];
        Outcome::done(Category::System, "Estado del equipo", ResponseBody::Rows { rows, note: None, attribution: None })
    }
}

#[async_trait]
impl CommandHandler for SystemHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::System(_))
    }

    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Outcome {
        let Intent::System(metric) = intent else { unreachable!("el enrutador comprueba la intención") };
        match metric {
            SystemMetric::Cpu => match self.system.cpu_percent().await {
                Reading::Value(percent) => Outcome::done(
                    Category::System,
                    format!("CPU al {} %", whole(percent)),
                    ResponseBody::Reading { value: whole(percent), unit: "%".into(), context: "de CPU en uso".into(), percent: Some(percent), rows: vec![], attribution: None },
                ),
                Reading::Unavailable(reason) => unavailable_outcome("CPU en uso", reason),
            },
            SystemMetric::Ram => match self.system.memory().await {
                Reading::Value(memory) => {
                    let percent = memory.used_bytes as f64 / memory.total_bytes.max(1) as f64 * 100.0;
                    Outcome::done(
                        Category::System,
                        format!("{} GB de RAM en uso", gigabytes(memory.used_bytes)),
                        ResponseBody::Reading {
                            value: gigabytes(memory.used_bytes),
                            unit: "GB".into(),
                            context: format!("en uso de {} GB", gigabytes(memory.total_bytes)),
                            percent: Some(percent),
                            rows: vec![DataRow::value("Disponible", format!("{} GB", gigabytes(memory.available_bytes)))],
                            attribution: None,
                        },
                    )
                }
                Reading::Unavailable(reason) => unavailable_outcome("RAM en uso", reason),
            },
            SystemMetric::Battery => match self.system.battery().await {
                Reading::Value(battery) => Outcome::done(
                    Category::System,
                    format!("Batería al {} %", whole(battery.percent)),
                    ResponseBody::Reading {
                        value: whole(battery.percent),
                        unit: "%".into(),
                        context: "de batería".into(),
                        percent: Some(battery.percent),
                        rows: vec![DataRow::text("Estado", battery.state)],
                        attribution: None,
                    },
                ),
                Reading::Unavailable(reason) => unavailable_outcome("Batería", reason),
            },
            SystemMetric::Internet => match self.system.internet().await {
                Reading::Value(connected) => {
                    let status = yes_no(connected, "Conectado a Internet", "Sin conexión a Internet");
                    Outcome::done(Category::System, status.clone(), ResponseBody::Rows { rows: vec![DataRow::text("Internet", status)], note: None, attribution: None })
                }
                Reading::Unavailable(reason) => unavailable_outcome("Internet", reason),
            },
            SystemMetric::Wifi => self.wifi().await,
            SystemMetric::DarkMode => match self.system.dark_mode().await {
                Reading::Value(dark) => {
                    let status = yes_no(dark, "Activado", "Desactivado");
                    Outcome::done(Category::System, format!("Modo oscuro: {status}"), ResponseBody::Rows { rows: vec![DataRow::text("Modo oscuro", status)], note: None, attribution: None })
                }
                Reading::Unavailable(reason) => unavailable_outcome("Modo oscuro", reason),
            },
            SystemMetric::Overview => self.overview().await,
        }
    }
}

pub struct FilesHandler {
    pub search: Arc<dyn FileSearchProvider>,
    pub settings: SettingsService,
    pub known_folders: Vec<AuthorizedFolder>,
}

impl FilesHandler {
    fn folder_labels(&self) -> Vec<String> {
        self.known_folders.iter().map(|folder| folder.path.to_string_lossy().into_owned()).collect()
    }
}

fn describe_query(query: &FileQuery) -> String {
    query.exact_name.clone().unwrap_or_else(|| query.terms.join(", "))
}

#[async_trait]
impl CommandHandler for FilesHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::SearchFiles(_))
    }

    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Outcome {
        let Intent::SearchFiles(query) = intent else { unreachable!("el enrutador comprueba la intención") };
        let settings = match self.settings.get() {
            Ok(settings) => settings,
            Err(error) => return Outcome::failed(Category::Files, "settings", error.to_string(), None),
        };
        if !settings.folders_confirmed {
            return Outcome::pending(
                Category::Files,
                "Pendiente de autorización",
                ResponseBody::FolderConsent { folders: self.folder_labels(), pending_request: context.request.clone() },
            );
        }
        let allowed: Vec<AuthorizedFolder> = self.known_folders.iter().filter(|folder| settings.authorized_folders.iter().any(|path| *path == folder.path.to_string_lossy())).cloned().collect();
        if allowed.is_empty() {
            return Outcome::failed(Category::Files, "noFolders", "No hay carpetas autorizadas para buscar.", Some("Autorízalas en Ajustes.".into()));
        }
        let outcome = self.search.search(query.clone(), allowed.clone(), context.cancel.clone()).await;
        let places = allowed.iter().map(|folder| folder.label.as_str()).collect::<Vec<_>>().join(" y ");
        if outcome.files.is_empty() && context.cancel.is_cancelled() {
            return Outcome::failed(Category::Files, "cancelled", "Búsqueda cancelada.", None);
        }
        if outcome.files.is_empty() {
            let text = format!("No encontré archivos con \u{201c}{}\u{201d} en {places}. Solo busco por nombre, sin abrir los archivos.", describe_query(query));
            return Outcome::done(Category::Files, "Sin coincidencias", ResponseBody::Text { text });
        }
        let count = outcome.files.len();
        let summary = if count == 1 { format!("1 coincidencia en {places}") } else { format!("{count} coincidencias en {places}") };
        let note = outcome.truncated.then(|| "Hay más coincidencias o carpetas muy grandes. Prueba con términos más concretos.".to_owned());
        Outcome::done(Category::Files, summary.clone(), ResponseBody::Files { summary, files: outcome.files, note })
    }
}

pub struct WeatherHandler {
    pub weather: Arc<dyn WeatherProvider>,
    pub settings: SettingsService,
}

fn attribution() -> Option<Attribution> {
    Some(Attribution { text: ATTRIBUTION_TEXT.into(), url: ATTRIBUTION_URL.into() })
}

#[async_trait]
impl CommandHandler for WeatherHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::Weather { .. })
    }

    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Outcome {
        let Intent::Weather { city, when } = intent else { unreachable!("el enrutador comprueba la intención") };
        let configured = self.settings.get().map(|settings| settings.weather_city).unwrap_or_default();
        let Some(city) = city.clone().or_else(|| (!configured.trim().is_empty()).then_some(configured)) else {
            return Outcome::pending(
                Category::Weather,
                "Falta la ciudad",
                ResponseBody::Clarification { question: "¿De qué ciudad quieres el clima? Dímelo en tu solicitud, o configura una en Ajustes.".into(), suggestions: Vec::<SuggestedRequest>::new() },
            );
        };
        let report = match self.weather.report(&city).await {
            Ok(report) => report,
            Err(WeatherError::CityNotFound) => return Outcome::failed(Category::Weather, "cityNotFound", format!("No encontré la ciudad \u{201c}{city}\u{201d}."), None),
            Err(_) => return Outcome::failed(Category::Weather, "weatherUnavailable", "No fue posible consultar el clima.", Some("Comprueba tu conexión a Internet. No muestro datos antiguos.".into())),
        };
        let day_row = |label: &str, day: &crate::providers::weather::DayForecast| {
            DataRow::value(label, format!("{} °C a {} °C · {}", whole(day.min_c), whole(day.max_c), day.condition))
        };
        let (mut outcome_value, mut unit, mut context, mut rows) = (decimal(report.temperature_c, 0), "°C".to_owned(), format!("{}, ahora en {}", report.condition, report.place), vec![day_row("Pronóstico de hoy", &report.today)]);
        match when {
            WeatherWhen::Now | WeatherWhen::Today => {}
            WeatherWhen::Tomorrow => {
                outcome_value = format!("{} a {}", whole(report.tomorrow.min_c), whole(report.tomorrow.max_c));
                unit = "°C".into();
                context = format!("{}, pronóstico de mañana en {}", report.tomorrow.condition, report.place);
                rows = vec![DataRow::value("Ahora", format!("{} °C · {}", whole(report.temperature_c), report.condition))];
            }
        }
        let summary = format!("{outcome_value} {unit} · {context}");
        Outcome {
            source: Some("Open-Meteo".into()),
            ..Outcome::done(Category::Weather, summary, ResponseBody::Reading { value: outcome_value, unit, context, percent: None, rows, attribution: attribution() })
        }
    }
}

pub struct ReminderIntentHandler {
    pub reminders: ReminderService,
}

fn local_iso(date: NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
    Local.from_local_datetime(&date.and_time(time)).earliest()
}

/// Convierte lo interpretado en un borrador. Nada se guarda hasta que el usuario confirma.
pub fn draft_reminder(request: &ReminderRequest, now: DateTime<Local>) -> ResponseBody {
    let content = request.content.clone();
    let due = request.date.zip(request.time).and_then(|(date, time)| local_iso(date, time));
    if let Some(question) = &request.ambiguity {
        return ResponseBody::ReminderDraft { content, due_at: due.map(|value| value.to_rfc3339()), question: Some(question.clone()), options: vec![] };
    }
    match (request.date, request.time, due) {
        (_, _, Some(value)) if value <= now => ResponseBody::ReminderDraft {
            content,
            due_at: None,
            question: Some(format!("El {} a las {} ya pasó. ¿Para cuándo lo quieres?", long_date(value.date_naive()), clock(&value))),
            options: vec![],
        },
        (_, _, Some(value)) => ResponseBody::ReminderDraft { content, due_at: Some(value.to_rfc3339()), question: None, options: vec![] },
        (Some(date), None, _) => {
            let options = [(8, "08:00"), (12, "12:00"), (18, "18:00")]
                .iter()
                .filter_map(|(hour, label)| {
                    let at = local_iso(date, NaiveTime::from_hms_opt(*hour, 0, 0)?)?;
                    (at > now).then(|| ReminderOption { label: (*label).to_owned(), due_at: at.to_rfc3339() })
                })
                .collect();
            ResponseBody::ReminderDraft { content, due_at: None, question: Some(format!("¿A qué hora del {} quieres el aviso?", long_date(date))), options }
        }
        _ => ResponseBody::ReminderDraft { content, due_at: None, question: Some("¿Qué día y a qué hora quieres el aviso?".into()), options: vec![] },
    }
}

#[async_trait]
impl CommandHandler for ReminderIntentHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::CreateReminder(_) | Intent::ListReminders)
    }

    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Outcome {
        match intent {
            Intent::CreateReminder(request) => {
                let body = draft_reminder(request, context.now);
                let summary = if let ResponseBody::ReminderDraft { question: Some(question), .. } = &body { question.clone() } else { format!("Por confirmar: {}", request.content) };
                Outcome::pending(Category::Reminders, summary, body)
            }
            _ => match self.reminders.list() {
                Ok(items) if items.is_empty() => Outcome::done(Category::Reminders, "Sin recordatorios", ResponseBody::Text { text: "No tienes recordatorios pendientes.".into() }),
                Ok(items) => {
                    let count = items.len();
                    let rows = items
                        .into_iter()
                        .map(|item| {
                            let due = DateTime::parse_from_rfc3339(&item.due_at).map_or_else(|_| item.due_at.clone(), |date| {
                                let local = date.with_timezone(&Local);
                                format!("{} · {}", long_date(local.date_naive()), clock(&local))
                            });
                            DataRow::value(&item.content, due)
                        })
                        .collect();
                    Outcome::done(Category::Reminders, format!("{count} recordatorios"), ResponseBody::Rows { rows, note: None, attribution: None })
                }
                Err(AppError::Database(_) | AppError::Operation(_) | AppError::Validation(_)) => Outcome::failed(Category::Reminders, "database", "No pude leer tus recordatorios.", None),
            },
        }
    }
}

/// Solo pide permiso: la captura ocurre únicamente tras la confirmación expresa en la interfaz.
pub struct ScreenHandler;

#[async_trait]
impl CommandHandler for ScreenHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::AnalyzeScreen { .. })
    }

    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Outcome {
        let Intent::AnalyzeScreen { question } = intent else { unreachable!("el enrutador comprueba la intención") };
        Outcome::pending(Category::Screen, "Pendiente de tu permiso para capturar", ResponseBody::CaptureRequest { question: question.clone() })
    }
}

pub struct ClarifyHandler;

#[async_trait]
impl CommandHandler for ClarifyHandler {
    fn can_handle(&self, intent: &Intent) -> bool {
        matches!(intent, Intent::Clarify { .. } | Intent::Unknown)
    }

    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Outcome {
        match intent {
            Intent::Clarify { question, suggestions } => {
                let suggestions = suggestions.iter().map(|request| SuggestedRequest { label: request.clone(), request: request.clone() }).collect();
                Outcome::pending(Category::General, question.clone(), ResponseBody::Clarification { question: question.clone(), suggestions })
            }
            _ => Outcome {
                status: InteractionStatus::Failed,
                ..Outcome::done(
                    Category::General,
                    "No entendí la solicitud",
                    ResponseBody::Text { text: "No entendí tu solicitud. Puedo decirte la hora, el clima o el estado de tu equipo, buscar archivos, crear recordatorios y analizar tu pantalla.".into() },
                )
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 5, 10, 0, 0).unwrap()
    }

    fn request(date: Option<(i32, u32, u32)>, time: Option<(u32, u32)>, ambiguity: Option<&str>) -> ReminderRequest {
        ReminderRequest {
            content: "Entregar".into(),
            date: date.and_then(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d)),
            time: time.and_then(|(h, m)| NaiveTime::from_hms_opt(h, m, 0)),
            ambiguity: ambiguity.map(str::to_owned),
        }
    }

    #[test]
    fn a_complete_date_only_needs_confirmation() {
        let ResponseBody::ReminderDraft { due_at, question, .. } = draft_reminder(&request(Some((2026, 11, 12)), Some((8, 0)), None), now()) else { panic!() };
        assert!(due_at.is_some());
        assert_eq!(question, None);
    }

    #[test]
    fn a_missing_hour_offers_options() {
        let ResponseBody::ReminderDraft { due_at, question, options, .. } = draft_reminder(&request(Some((2026, 11, 12)), None, None), now()) else { panic!() };
        assert_eq!(due_at, None);
        assert!(question.unwrap().contains("jueves 12 de noviembre"));
        assert_eq!(options.len(), 3);
    }

    #[test]
    fn options_already_past_are_not_offered() {
        let ResponseBody::ReminderDraft { options, .. } = draft_reminder(&request(Some((2026, 11, 5)), None, None), now()) else { panic!() };
        assert_eq!(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["12:00", "18:00"]);
    }

    #[test]
    fn a_missing_date_is_asked_for() {
        let ResponseBody::ReminderDraft { question, .. } = draft_reminder(&request(None, None, None), now()) else { panic!() };
        assert!(question.is_some());
    }

    #[test]
    fn a_past_date_is_not_accepted() {
        let ResponseBody::ReminderDraft { due_at, question, .. } = draft_reminder(&request(Some((2026, 11, 4)), Some((8, 0)), None), now()) else { panic!() };
        assert_eq!(due_at, None);
        assert!(question.unwrap().contains("ya pasó"));
    }

    #[test]
    fn the_model_ambiguity_is_passed_on() {
        let ResponseBody::ReminderDraft { question, .. } = draft_reminder(&request(Some((2026, 11, 12)), Some((8, 0)), Some("¿A las 8 de la mañana o de la noche?")), now()) else { panic!() };
        assert_eq!(question.as_deref(), Some("¿A las 8 de la mañana o de la noche?"));
    }

    #[tokio::test]
    async fn the_time_comes_from_the_clock() {
        let context = HandlerContext { request: "hora".into(), now: now() + Duration::hours(4), cancel: CancellationToken::new() };
        let outcome = TimeHandler.handle(&Intent::Time, &context).await;
        assert_eq!(outcome.summary, "14:00 · jueves 5 de noviembre");
    }
}
