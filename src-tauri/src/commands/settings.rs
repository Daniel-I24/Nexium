//! Ajustes: lo que se muestra en la pantalla y su validación al guardar.

use serde::Serialize;
use tauri::State;

use super::{weather, AppState};
use crate::{
    error::AppResult,
    providers::{
        ai::AiServiceStatus,
        speech::{self, SpeechModelStatus},
    },
    services::settings::Settings,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    display_name: String,
    data_notice: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    settings: Settings,
    provider: ProviderInfo,
    ai_status: AiServiceStatus,
    ai_requests_today: u32,
    tokens_today: u64,
    known_folders: Vec<String>,
    speech_models: Vec<SpeechModelStatus>,
    /// Lo que elegiría el modo automático en este equipo, y por qué.
    speech_auto: speech::AutoChoice,
    microphones: Vec<speech::InputDevice>,
    /// El dispositivo que Nexium usará al pulsar Hablar con los ajustes guardados.
    active_microphone: Option<String>,
}

fn settings_view(state: &AppState) -> AppResult<SettingsView> {
    let settings = state.settings.get()?;
    let (ai_requests_today, tokens_today) = state.usage.today_totals()?;
    let hardware = speech::Hardware::detect();
    let (auto_model, reason) = speech::auto_choice(hardware);
    let active = speech::resolve_model(&settings.speech_model, hardware).unwrap_or(auto_model);
    Ok(SettingsView {
        speech_models: speech::statuses(&state.models_dir, active.id),
        speech_auto: speech::AutoChoice {
            model_id: auto_model.id.into(),
            display_name: auto_model.display_name.into(),
            reason,
        },
        microphones: speech::input_devices(),
        active_microphone: speech::active_input(settings.microphone.as_deref()),
        settings,
        provider: ProviderInfo {
            display_name: state.ai.display_name().into(),
            data_notice: state.ai.data_notice().into(),
        },
        ai_status: state.assistant.ai_status.current(state.ai.is_configured()),
        ai_requests_today,
        tokens_today,
        known_folders: state
            .known_folders
            .iter()
            .map(|folder| folder.path.to_string_lossy().into_owned())
            .collect(),
    })
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<SettingsView> {
    settings_view(&state)
}

/// La ciudad del clima, si cambia, se comprueba con Open-Meteo y se guarda con su nombre oficial.
#[tauri::command]
pub async fn settings_save(
    state: State<'_, AppState>,
    mut settings: Settings,
) -> AppResult<SettingsView> {
    settings.validate(&speech::catalog_ids())?;
    let city = settings.weather_city.trim().to_owned();
    if city.is_empty() {
        settings.weather_city.clear();
    } else if city != state.settings.get()?.weather_city {
        settings.weather_city = weather::verified_city(state.weather.as_ref(), &city).await?;
    }
    state.settings.save_editable(&settings)?;
    settings_view(&state)
}
