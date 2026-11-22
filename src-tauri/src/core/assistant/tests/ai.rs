//! Uso de la IA: cuándo se llama, cuota, regiones y el mensaje de cada error.

use super::*;

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
    let now = chrono::Local
        .with_ymd_and_hms(2026, 11, 22, 18, 30, 0)
        .unwrap();
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
