//! Varias solicitudes en una misma petición: cada una en su bloque, una sola entrada en el historial.

use super::*;

fn parts(response: &AssistantResponse) -> &[PartResponse] {
    match &response.body {
        ResponseBody::Multiple { parts } => parts,
        other => panic!("se esperaban varias partes: {other:?}"),
    }
}

async fn ask_by_voice(assistant: &Assistant, transcript: &str) -> AssistantResponse {
    assistant
        .process_timed(
            transcript,
            None,
            CancellationToken::new(),
            silent(),
            Timings::new("voz"),
        )
        .await
}

#[tokio::test]
async fn wifi_and_weather_are_both_answered_in_order() {
    let h = harness(None, true, false);
    let response = ask(
        &h.assistant,
        "¿A qué red Wi-Fi estoy conectado? Y dame el clima de la ciudad en la que me encuentro",
    )
    .await;
    let parts = parts(&response);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].category, Category::System);
    assert_eq!(parts[0].request, "A qué red Wi-Fi estoy conectado");
    // Sin ciudad en Ajustes, el clima la pregunta: no bloquea la parte del Wi-Fi.
    assert!(matches!(
        parts[1].body,
        ResponseBody::WeatherCityQuestion { .. }
    ));
    assert_eq!(parts[1].status, InteractionStatus::Pending);
    assert_eq!(response.status, InteractionStatus::Pending);
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].parts.as_ref().unwrap().len(), 2);
}

#[tokio::test]
async fn three_local_requests_by_text_and_by_voice() {
    for (by_voice, text) in [
        (
            false,
            "¿Qué hora es? ¿Cuánta batería tengo? ¿Cuánta RAM uso?",
        ),
        (true, "Qué hora es, cuánta batería tengo y cuánta RAM uso."),
    ] {
        let h = harness(None, true, false);
        let response = if by_voice {
            ask_by_voice(&h.assistant, text).await
        } else {
            ask(&h.assistant, text).await
        };
        let parts = parts(&response);
        assert_eq!(
            parts.iter().map(|part| part.category).collect::<Vec<_>>(),
            [Category::Time, Category::System, Category::System],
            "{text}"
        );
        assert_eq!(response.status, InteractionStatus::Completed);
        for part in parts {
            assert!(
                response.summary.contains(&part.summary),
                "{}",
                response.summary
            );
        }
        assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
        assert_eq!(h.assistant.history.list(None).unwrap().len(), 1);
    }
}

#[tokio::test]
async fn a_failed_part_does_not_stop_the_others_and_says_why() {
    let h = harness(None, true, false);
    let response = ask(
        &h.assistant,
        "¿Qué hora es y cómo está el clima en Nolugar?",
    )
    .await;
    let parts = parts(&response);
    assert_eq!(parts[0].status, InteractionStatus::Completed);
    assert_eq!(parts[1].status, InteractionStatus::Failed);
    assert!(
        matches!(parts[1].body, ResponseBody::Error { ref message, .. } if message.contains("Nolugar"))
    );
    assert_eq!(response.status, InteractionStatus::Completed);
    assert!(response.summary.contains("(falló)"));
}

#[tokio::test]
async fn the_ai_splits_what_the_rules_cannot_and_a_capture_waits_for_consent() {
    let h = harness_with_actions(
        Some(Ok(vec![
            Action {
                request: "qué hora es".into(),
                intent: Intent::Time,
            },
            Action {
                request: "explícame este error".into(),
                intent: Intent::AnalyzeScreen {
                    question: "explícame este error".into(),
                },
            },
        ])),
        true,
        false,
    );
    let response = ask(
        &h.assistant,
        "Dame la hora y explícame este error de la ventana",
    )
    .await;
    let parts = parts(&response);
    assert_eq!(parts[0].category, Category::Time);
    assert!(matches!(parts[1].body, ResponseBody::CaptureRequest { .. }));
    // Nunca se captura sin permiso: la parte espera, y la hora ya está respondida.
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
    assert_eq!(parts[0].status, InteractionStatus::Completed);
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn completing_a_part_updates_only_that_part_of_the_same_entry() {
    let h = harness(None, true, false);
    let response = ask(
        &h.assistant,
        "¿Cuánta batería tengo? Y dame el clima de la ciudad en la que me encuentro",
    )
    .await;
    let continued = h
        .assistant
        .process_timed(
            "clima en Bogotá",
            Continues {
                id: Some(response.id),
                part: Some(1),
            },
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert_eq!(continued.id, response.id);
    assert_eq!(continued.part, Some(1));
    assert_eq!(continued.status, InteractionStatus::Completed);
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, InteractionStatus::Completed);
    let stored = history[0].parts.as_ref().unwrap();
    assert_eq!(
        stored[1].request,
        "dame el clima de la ciudad en la que me encuentro"
    );
    assert_eq!(stored[1].status, InteractionStatus::Completed);
}

#[tokio::test]
async fn a_capture_for_one_part_updates_that_part() {
    let h = harness_with_actions(
        Some(Ok(vec![
            Action {
                request: "qué hora es".into(),
                intent: Intent::Time,
            },
            Action {
                request: "qué hay en mi pantalla".into(),
                intent: Intent::AnalyzeScreen {
                    question: "qué hay en mi pantalla".into(),
                },
            },
        ])),
        true,
        false,
    );
    let response = ask(
        &h.assistant,
        "Dame la hora y dime qué hay en mi pantalla ahora",
    )
    .await;
    let analyzed = h
        .assistant
        .analyze_screen(
            "qué hay en mi pantalla",
            Continues {
                id: Some(response.id),
                part: Some(1),
            },
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(analyzed.part, Some(1));
    assert!(matches!(analyzed.body, ResponseBody::ScreenAnalysis { .. }));
    let entry = &h.assistant.history.list(None).unwrap()[0];
    assert_eq!(entry.status, InteractionStatus::Completed);
}

#[tokio::test]
async fn continuing_a_part_never_splits_the_new_text() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "¿Qué hora es y cuánta batería tengo?").await;
    let continued = h
        .assistant
        .process_timed(
            "¿Qué hora es y cuánta RAM uso?",
            Continues {
                id: Some(response.id),
                part: Some(0),
            },
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert!(!matches!(continued.body, ResponseBody::Multiple { .. }));
}

#[tokio::test]
async fn more_than_four_requests_answer_four_and_say_so() {
    let h = harness(None, true, false);
    let response = ask(
        &h.assistant,
        "¿Qué hora es? ¿Batería? ¿Cuánta RAM uso? ¿Uso de CPU? ¿Estoy conectado a internet?",
    )
    .await;
    let parts = parts(&response);
    assert_eq!(parts.len(), 5);
    assert!(
        matches!(parts[4].body, ResponseBody::Error { ref message, .. } if message.contains("Atiendo hasta 4 solicitudes"))
    );
}

#[tokio::test]
async fn a_single_request_keeps_its_usual_answer() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "¿Qué hora es?").await;
    assert!(!matches!(response.body, ResponseBody::Multiple { .. }));
    assert_eq!(response.part, None);
    assert!(h.assistant.history.list(None).unwrap()[0].parts.is_none());
}
