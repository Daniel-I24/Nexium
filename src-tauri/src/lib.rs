pub mod commands;
pub mod core;
pub mod database;
pub mod error;
pub mod models;
pub mod platform;
pub mod providers;
pub mod services;

use std::{sync::Arc, sync::Mutex, time::Duration};

use chrono::Utc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, Theme, WindowEvent,
};
use tokio_util::sync::CancellationToken;

use crate::{
    commands::AppState,
    core::{
        assistant::Assistant,
        handlers::{
            ClarifyHandler, CommandHandler, DocumentsHandler, FilesHandler, NavigationHandler,
            ReminderIntentHandler, ScreenHandler, SystemHandler, TimeHandler, WeatherHandler,
        },
    },
    database::Database,
    models::Reading,
    platform::WindowAwareCapture,
    providers::{
        ai::{server::NexiumServer, AiProvider, AiStatusTracker},
        documents::DocumentOffers,
        files::{AuthorizedFolder, LocalFileSearch},
        notification::{reminder_title, NotificationProvider, TauriNotifications},
        speech::{SpeechProvider, WhisperSpeech},
        system::NativeSystemProvider,
        weather::{OpenMeteo, WeatherProvider},
    },
    services::{
        history::HistoryService, performance::PerformanceLog, reminders::ReminderService,
        settings::SettingsService, usage::UsageService,
    },
};

/// Clave del aviso que explica, una sola vez, que cerrar la ventana no cierra Nexium.
const TRAY_NOTICE: &str = "tray-notice";

#[cfg(target_os = "macos")]
const TRAY_NOTICE_BODY: &str = "Nexium sigue activo en la barra de menús y te avisará de tus recordatorios. Para cerrarlo del todo, abre su icono y elige Salir.";
#[cfg(not(target_os = "macos"))]
const TRAY_NOTICE_BODY: &str = "Nexium sigue activo en la bandeja del sistema y te avisará de tus recordatorios. Para cerrarlo del todo, haz clic derecho en su icono y elige Salir.";

const REMINDER_CHECK_INTERVAL: Duration = Duration::from_secs(20);
const SPEECH_IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(30);

fn known_folders<R: Runtime>(app: &AppHandle<R>) -> Vec<AuthorizedFolder> {
    let paths = app.path();
    [
        (paths.document_dir(), "Documentos"),
        (paths.desktop_dir(), "Escritorio"),
    ]
    .into_iter()
    .filter_map(|(path, label)| {
        path.ok().map(|path| AuthorizedFolder {
            path,
            label: label.into(),
        })
    })
    .collect()
}

/// La dirección del servidor se fija al compilar. El adaptador directo solo existe en desarrollo y solo actúa sin servidor.
fn build_ai(install_id: String) -> Arc<dyn AiProvider> {
    let server_url = option_env!("NEXIUM_AI_SERVER_URL").filter(|url| !url.trim().is_empty());
    #[cfg(debug_assertions)]
    if let (None, Some(key)) = (server_url, providers::ai::gemini::dev_key()) {
        return Arc::new(providers::ai::gemini::GeminiProvider::new(key));
    }
    Arc::new(NexiumServer::new(server_url, install_id))
}

fn build_state<R: Runtime>(app: &AppHandle<R>) -> Result<AppState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db = Database::open(&data_dir.join("nexium.db"))?;
    let settings = SettingsService::new(db.clone());
    let history = HistoryService::new(db.clone());
    let reminders = ReminderService::new(db.clone());
    let usage = UsageService::new(db);
    let ai = build_ai(settings.install_id()?);
    let folders = known_folders(app);
    let offers = Arc::new(DocumentOffers::default());
    let weather: Arc<dyn WeatherProvider> = Arc::new(OpenMeteo::default());

    let theme_app = app.clone();
    let system = Arc::new(NativeSystemProvider::new(Arc::new(move || match theme_app
        .get_webview_window("main")
        .map(|window| window.theme())
    {
        Some(Ok(Theme::Dark)) => Reading::Value(true),
        Some(Ok(_)) => Reading::Value(false),
        _ => Reading::Unavailable("Este sistema no indica si el modo oscuro está activo.".into()),
    })));
    system.warm_up();
    let handlers: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(TimeHandler),
        Box::new(SystemHandler { system }),
        Box::new(FilesHandler {
            search: Arc::new(LocalFileSearch),
            settings: settings.clone(),
            known_folders: folders.clone(),
        }),
        Box::new(DocumentsHandler {
            files: FilesHandler {
                search: Arc::new(LocalFileSearch),
                settings: settings.clone(),
                known_folders: folders.clone(),
            },
            offers: offers.clone(),
            provider: ai.display_name().to_owned(),
        }),
        Box::new(WeatherHandler {
            weather: weather.clone(),
            settings: settings.clone(),
        }),
        Box::new(ReminderIntentHandler {
            reminders: reminders.clone(),
        }),
        Box::new(ScreenHandler),
        Box::new(NavigationHandler),
        Box::new(ClarifyHandler),
    ];
    let assistant = Arc::new(Assistant {
        ai: ai.clone(),
        ai_status: AiStatusTracker::default(),
        capture: Arc::new(WindowAwareCapture::new(app.clone())),
        handlers,
        settings: settings.clone(),
        history: history.clone(),
        usage: usage.clone(),
        performance: PerformanceLog::new(data_dir.join("rendimiento.log")),
    });
    Ok(AppState {
        assistant,
        ai,
        speech: Arc::new(WhisperSpeech::default()),
        settings,
        history,
        reminders,
        usage,
        known_folders: folders,
        offers,
        weather,
        models_dir: data_dir.join("models"),
        cancel: Mutex::new(CancellationToken::new()),
        recording: Mutex::new(None),
        downloading: Mutex::new(None),
    })
}

/// Avisa de los recordatorios vencidos mientras Nexium esté en ejecución, incluso en la bandeja.
fn start_reminder_scheduler<R: Runtime>(app: AppHandle<R>, reminders: ReminderService) {
    let notifications = TauriNotifications::new(app.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            if let Ok(due) = reminders.take_due(Utc::now()) {
                for reminder in &due.to_notify {
                    let sent = notifications
                        .notify_reminder(
                            &reminder_title(&reminder.content),
                            &reminder.content,
                            reminder.id,
                        )
                        .is_ok();
                    let _ = reminders.mark_notified(reminder.id, sent);
                }
                // También cuando solo hubo vencidos: la pantalla de Recordatorios cambia de estado.
                if due.changed() {
                    let _ = app.emit("reminders://changed", ());
                }
            }
            tokio::time::sleep(REMINDER_CHECK_INTERVAL).await;
        }
    });
}

/// Devuelve al sistema la memoria del modelo de voz cuando lleva tiempo sin usarse.
fn start_speech_release(speech: Arc<dyn SpeechProvider>, settings: SettingsService) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(SPEECH_IDLE_CHECK_INTERVAL).await;
            if let Ok(current) = settings.get() {
                speech.release_idle(Duration::from_secs(
                    u64::from(current.speech_idle_minutes) * 60,
                ));
            }
        }
    });
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Mostrar Nexium", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Nexium")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = build_state(&handle)?;
            start_reminder_scheduler(handle.clone(), state.reminders.clone());
            start_speech_release(state.speech.clone(), state.settings.clone());
            app.manage(state);
            // Sin bandeja disponible, cerrar la ventana cierra la aplicación.
            let has_tray = build_tray(&handle).is_ok();
            if let Some(window) = app.get_webview_window("main") {
                platform::window_size::fit_to_screen(&window);
                let hidden = window.clone();
                let settings = app.state::<AppState>().settings.clone();
                let notices = TauriNotifications::new(handle.clone());
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if has_tray {
                            api.prevent_close();
                            let _ = hidden.hide();
                            // Solo la primera vez: después, la persona ya sabe dónde sigue Nexium.
                            if settings.first_time(TRAY_NOTICE).unwrap_or(false) {
                                let _ = notices.notify("Nexium sigue activo", TRAY_NOTICE_BODY);
                            }
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::assistant_submit,
            commands::assistant_cancel,
            commands::folders::folders_allow_known,
            commands::folders::folders_pick,
            commands::folders::folders_remove,
            commands::documents::document_analyze,
            commands::documents::document_pick,
            commands::weather::weather_city_set,
            commands::updates::update_check,
            commands::updates::update_install,
            commands::open_attribution,
            commands::voice::voice_start,
            commands::voice::voice_stop,
            commands::voice::voice_cancel,
            commands::voice::speech_model_download,
            commands::screen_analyze,
            commands::records::reminders_list,
            commands::records::reminders_create,
            commands::records::reminders_update,
            commands::records::reminders_delete,
            commands::records::history_list,
            commands::records::history_delete,
            commands::records::history_clear,
            commands::records::history_cancel,
            commands::records::history_record_failure,
            commands::settings::settings_get,
            commands::settings::settings_save,
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar Nexium");
}
