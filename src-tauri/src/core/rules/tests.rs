use super::*;

fn system(metric: SystemMetric) -> Option<Intent> {
    Some(Intent::System(metric))
}

#[test]
fn recognizes_local_questions_without_ai() {
    let cases = [
        ("¿Qué hora es?", Some(Intent::Time)),
        ("Dime la hora", Some(Intent::Time)),
        ("¿Qué día es hoy?", Some(Intent::Time)),
        (
            "¿Cuánta memoria RAM estoy utilizando?",
            system(SystemMetric::Ram),
        ),
        ("cuanta ram tengo libre", system(SystemMetric::Ram)),
        ("¿Cómo va el procesador?", system(SystemMetric::Cpu)),
        ("uso de CPU", system(SystemMetric::Cpu)),
        ("¿Cuánta batería me queda?", system(SystemMetric::Battery)),
        (
            "¿Estoy conectado a Internet?",
            system(SystemMetric::Internet),
        ),
        (
            "¿A qué red Wi-Fi estoy conectado?",
            system(SystemMetric::Wifi),
        ),
        (
            "¿Está activado el modo oscuro?",
            system(SystemMetric::DarkMode),
        ),
        (
            "¿Cuál es el estado del sistema?",
            system(SystemMetric::Overview),
        ),
    ];
    for (phrase, expected) in cases {
        assert_eq!(match_local(phrase), expected, "{phrase}");
    }
}

#[test]
fn tolerates_frequent_variants_and_slight_transcription_errors() {
    let cases = [
        ("¿Cuánta batería tengo?", system(SystemMetric::Battery)),
        ("cuanta bateria tengo", system(SystemMetric::Battery)),
        ("CUÁNTA BATERÍA TENGO", system(SystemMetric::Battery)),
        ("cuánta vateria me queda", system(SystemMetric::Battery)),
        ("¿Me alcanza la pila?", system(SystemMetric::Battery)),
        ("cuanta ram", system(SystemMetric::Ram)),
        ("Cuánta memoria RAM me queda.", system(SystemMetric::Ram)),
        ("¿Cómo va el procesadr?", system(SystemMetric::Cpu)),
        ("estoy conectado", system(SystemMetric::Internet)),
        (
            "¿Estoy conectado a Internet?",
            system(SystemMetric::Internet),
        ),
        (
            "¿A qué red wifi estoy conectado?",
            system(SystemMetric::Wifi),
        ),
        ("estado del wi fi", system(SystemMetric::Wifi)),
        ("¿Tengo el guifi encendido?", system(SystemMetric::Wifi)),
        // Transcripción real: Wi-Fi pegado a la palabra anterior.
        ("Aquí RedWiFi estoy conectado.", system(SystemMetric::Wifi)),
        (
            "¿A qué redwifi estoy conectado?",
            system(SystemMetric::Wifi),
        ),
        (
            "¿Está activado el modo obscuro?",
            system(SystemMetric::DarkMode),
        ),
        ("Oye, ¿qué hora es?", Some(Intent::Time)),
        ("¿Qué horas son?", Some(Intent::Time)),
        ("¿Qué horas es?", Some(Intent::Time)),
        ("dime la fecha de hoy", Some(Intent::Time)),
    ];
    for (phrase, expected) in cases {
        assert_eq!(match_local(phrase), expected, "{phrase}");
    }
}

#[test]
fn document_requests_are_recognized_without_ai() {
    let doc = |task, terms: &[&str]| {
        Some(Intent::Document {
            task,
            query: FileQuery {
                exact_name: None,
                terms: terms.iter().map(|term| (*term).to_owned()).collect(),
            },
        })
    };
    assert_eq!(
        match_local("Resume mi documento de bases de datos"),
        doc(DocumentTask::Summarize, &["bases", "datos"])
    );
    assert_eq!(
        match_local("Analiza este archivo"),
        doc(DocumentTask::Analyze, &[])
    );
    assert_eq!(
        match_local("Hazme un resumen del informe de programación"),
        doc(DocumentTask::Summarize, &["programacion"])
    );
    assert_eq!(
        match_local("¿Qué dice el archivo acta.docx?"),
        Some(Intent::Document {
            task: DocumentTask::Summarize,
            query: FileQuery {
                exact_name: Some("acta.docx".into()),
                terms: vec![]
            }
        })
    );
    // Sin un documento de por medio no es esta operación.
    assert!(!matches!(
        match_local("Explícame este error"),
        Some(Intent::Document { .. })
    ));
}

#[test]
fn opens_sections_without_ai() {
    for (phrase, section) in [
        ("Abre los recordatorios.", Section::Reminders),
        ("abre el historial", Section::History),
        ("Ve a Ajustes", Section::Settings),
        ("ir a la configuración", Section::Settings),
        ("Llévame al asistente", Section::Assistant),
    ] {
        assert_eq!(
            match_local(phrase),
            Some(Intent::OpenSection(section)),
            "{phrase}"
        );
    }
    // Preguntar por los recordatorios los muestra en la conversación; no cambia de sección.
    assert_eq!(
        match_local("¿Qué recordatorios tengo?"),
        Some(Intent::ListReminders)
    );
}

#[test]
fn a_reminder_transcribed_as_recuerda_mi_is_still_a_reminder() {
    // Sin fecha numérica ambigua, la orden sigue a la IA para extraer contenido y fecha.
    assert_eq!(
        match_local("Recuerda mi llamar a mamá mañana a las cinco."),
        None
    );
    assert!(matches!(
        match_local("Recuerda mi pagar el 5/6 a las 10:00"),
        Some(Intent::Clarify { .. })
    ));
}

#[test]
fn short_words_are_not_corrected_into_others() {
    assert_eq!(prepare("ahora"), "ahora");
    assert_eq!(prepare("historia del arte"), "historia del arte");
    assert_eq!(prepare("Bacteria"), "bateria");
    assert_eq!(edit_distance("procesadr", "procesador"), 1);
}

#[test]
fn a_phrase_about_where_i_am_is_not_a_city() {
    for phrase in [
        "Dame el clima en la ciudad en la que me encuentro",
        "¿Qué temperatura hace aquí?",
        "¿Cómo está el clima en mi ciudad?",
        "¿Va a llover en donde estoy?",
    ] {
        assert!(
            matches!(
                match_local(phrase),
                Some(Intent::Weather { city: None, .. })
            ),
            "{phrase}"
        );
    }
    assert!(is_place_name("el cairo"));
    assert!(is_place_name("san jose de las lajas"));
    assert!(!is_place_name("la ciudad en la que me encuentro"));
}

#[test]
fn the_city_keeps_how_it_was_written() {
    assert_eq!(
        match_local("clima en nueva york"),
        Some(Intent::Weather {
            city: Some("nueva york".into()),
            when: WeatherWhen::Now
        })
    );
}

#[test]
fn recognizes_weather_with_city_and_day() {
    assert_eq!(
        match_local("¿Cómo está el clima hoy?"),
        Some(Intent::Weather {
            city: None,
            when: WeatherWhen::Today
        })
    );
    assert_eq!(
        match_local("¿Va a llover mañana en Bogotá?"),
        Some(Intent::Weather {
            city: Some("Bogotá".into()),
            when: WeatherWhen::Tomorrow
        })
    );
}

#[test]
fn a_named_city_is_always_recognized() {
    let city = |phrase: &str| match match_local(phrase) {
        Some(Intent::Weather { city, .. }) => city,
        other => panic!("{phrase}: {other:?}"),
    };
    for (phrase, expected) in [
        ("Como esta el clima de Bogota?", "Bogota"),
        ("¿Cómo está el clima de Bogotá?", "Bogotá"),
        ("clima de bogota", "bogota"),
        ("CLIMA DE BOGOTA", "BOGOTA"),
        ("clima en medellin", "medellin"),
        ("clima para cali", "cali"),
        ("¿Qué tiempo hace en Medellín?", "Medellín"),
        ("¿Qué temperatura hace en Pasto?", "Pasto"),
        ("¿Va a llover mañana en Bogotá?", "Bogotá"),
        ("¿Va a llover en Cali mañana?", "Cali"),
        ("¿Está lloviendo en Manizales?", "Manizales"),
        ("Pronóstico para Cartagena", "Cartagena"),
        ("pronostico del clima para barranquilla", "barranquilla"),
        ("dime el clima de hoy en Cali", "Cali"),
        ("clima de mañana en Tunja", "Tunja"),
        ("clima en Santa Marta hoy, por favor", "Santa Marta"),
        ("clima de Bogota por favor", "Bogota"),
        ("¿Cómo está el clima en Nueva York?", "Nueva York"),
        (
            "clima en San José de las Lajas del Río Grande",
            "San José de las Lajas del Río Grande",
        ),
        ("¿Qué temperatura hace en La Paz?", "La Paz"),
        ("la temperatura en la paz", "la paz"),
        ("Clima de Ciudad de México", "Ciudad de México"),
        ("temperatura de buenos aires", "buenos aires"),
        ("clima en el cairo", "el cairo"),
    ] {
        assert_eq!(city(phrase).as_deref(), Some(expected), "{phrase}");
    }
}

#[test]
fn without_a_named_city_the_saved_one_is_used() {
    for phrase in [
        "¿Cómo está el clima?",
        "clima de hoy",
        "clima para mañana",
        "¿Cómo está el clima hoy?",
        "pronóstico de lluvia",
        "¿Qué tiempo hace ahora?",
        "clima en la ciudad",
        "clima de esta ciudad",
    ] {
        assert!(
            matches!(
                match_local(phrase),
                Some(Intent::Weather { city: None, .. })
            ),
            "{phrase}"
        );
    }
}

#[test]
fn recognizes_screen_requests() {
    for phrase in [
        "¿Qué aparece en mi pantalla?",
        "Explícame este error",
        "¿Qué significa este mensaje?",
        "Ayúdame a entender esta interfaz",
    ] {
        assert!(
            matches!(match_local(phrase), Some(Intent::AnalyzeScreen { .. })),
            "{phrase}"
        );
    }
}

#[test]
fn exact_file_names_do_not_need_ai() {
    assert_eq!(
        match_local("¿Existe el archivo proyecto.pdf?"),
        Some(Intent::SearchFiles(FileQuery {
            exact_name: Some("proyecto.pdf".into()),
            terms: vec![]
        }))
    );
}

#[test]
fn leaves_ambiguous_phrases_to_the_ai() {
    for phrase in [
        "Recuérdame entregar el proyecto mañana",
        "Recuérdame revisar la memoria del informe el lunes",
        "Necesito encontrar el informe que hice para la clase de programación",
        "Busca mi documento de bases de datos",
        "cuéntame un chiste",
    ] {
        assert_eq!(match_local(phrase), None, "{phrase}");
    }
}

#[test]
fn a_numeric_date_with_two_readings_is_asked_without_ai() {
    assert_eq!(
        match_local("Recuérdame pagar el 5/6 a las 10:00"),
        Some(Intent::Clarify {
            question: "¿Te refieres al 5 de junio o al 6 de mayo?".into(),
            suggestions: vec![
                "Recuérdame pagar el 5 de junio a las 10:00".into(),
                "Recuérdame pagar el 6 de mayo a las 10:00".into()
            ],
        })
    );
}

#[test]
fn numeric_date_readings() {
    let question = |phrase: &str| match ambiguous_date_question(phrase) {
        Some(Intent::Clarify { question, .. }) => Some(question),
        _ => None,
    };
    for (phrase, expected) in [
        ("el 5/6", "¿Te refieres al 5 de junio o al 6 de mayo?"),
        ("el 05/06", "¿Te refieres al 5 de junio o al 6 de mayo?"),
        ("el 5-6", "¿Te refieres al 5 de junio o al 6 de mayo?"),
        (
            "el 1/12/2027",
            "¿Te refieres al 1 de diciembre de 2027 o al 12 de enero de 2027?",
        ),
        (
            "el 3-4-27",
            "¿Te refieres al 3 de abril de 2027 o al 4 de marzo de 2027?",
        ),
        (
            "del 13/6 al 7/8",
            "¿Te refieres al 7 de agosto o al 8 de julio?",
        ),
    ] {
        assert_eq!(question(phrase).as_deref(), Some(expected), "{phrase}");
    }
    for phrase in [
        "el 13/6",
        "el 6/13",
        "el 5/5",
        "el 0/6",
        "el 2026-06-05",
        "las páginas 5-6",
        "el 005/6",
        "el 5/6-7",
        "v5/6",
        "el 12 de mayo",
    ] {
        assert_eq!(question(phrase), None, "{phrase}");
    }
}

#[test]
fn local_terms_drop_empty_words() {
    assert_eq!(
        local_search_terms("Busca mi informe de programación"),
        ["informe", "programacion"]
    );
}

#[test]
fn normalize_removes_accents_and_signs() {
    assert_eq!(normalize("¿Cuánta   BATERÍA?"), "cuanta bateria");
}

/// Qué intención reconoce cada parte, en orden. `None`: la petición no se divide en local.
fn compound_kinds(text: &str) -> Option<Vec<&'static str>> {
    match split_compound(text) {
        Compound::Local { actions, .. } => Some(
            actions
                .iter()
                .map(|action| match action.intent {
                    Intent::Time => "hora",
                    Intent::System(SystemMetric::Wifi) => "wifi",
                    Intent::System(SystemMetric::Battery) => "batería",
                    Intent::System(SystemMetric::Ram) => "ram",
                    Intent::System(SystemMetric::Cpu) => "cpu",
                    Intent::System(SystemMetric::Internet) => "internet",
                    Intent::System(SystemMetric::DarkMode) => "modo oscuro",
                    Intent::System(_) => "sistema",
                    Intent::Weather { .. } => "clima",
                    Intent::ListReminders => "recordatorios",
                    Intent::AnalyzeScreen { .. } => "pantalla",
                    Intent::OpenSection(_) => "sección",
                    Intent::Document { .. } => "documento",
                    _ => "otra",
                })
                .collect(),
        ),
        _ => None,
    }
}

#[test]
fn several_requests_in_one_are_split_in_order() {
    for (text, expected) in [
        (
            "¿A qué red Wi-Fi estoy conectado? Y dame el clima de la ciudad en la que me encuentro",
            vec!["wifi", "clima"],
        ),
        (
            "¿Qué hora es y cuánta batería tengo?",
            vec!["hora", "batería"],
        ),
        (
            "Qué hora es y cuánta batería tengo.",
            vec!["hora", "batería"],
        ),
        ("¿Cuánta RAM estoy usando? ¿Y la CPU?", vec!["ram", "cpu"]),
        (
            "dime la hora, la batería y la RAM",
            vec!["hora", "batería", "ram"],
        ),
        (
            "¿Estoy conectado a internet? Además, ¿qué hora es?",
            vec!["internet", "hora"],
        ),
        (
            "¿Cuánta batería tengo? También dime la hora",
            vec!["batería", "hora"],
        ),
        ("clima en Bogotá y qué hora es", vec!["clima", "hora"]),
        ("clima de Pasto y clima de Bogotá", vec!["clima", "clima"]),
        (
            "¿qué recordatorios tengo? y ¿qué hora es?",
            vec!["recordatorios", "hora"],
        ),
        (
            "¿Está activado el modo oscuro y cuánta batería me queda?",
            vec!["modo oscuro", "batería"],
        ),
        (
            "¿Qué hora es? ¿Cuánta batería tengo? ¿Cuánta RAM uso?",
            vec!["hora", "batería", "ram"],
        ),
        (
            "Uso de CPU, uso de RAM y nivel de batería",
            vec!["cpu", "ram", "batería"],
        ),
        (
            "¿Qué red wifi tengo y estoy conectado a internet?",
            vec!["wifi", "internet"],
        ),
        (
            "Dime la hora y el clima en Cartagena",
            vec!["hora", "clima"],
        ),
        (
            "¿Qué aparece en mi pantalla? Y dime la hora",
            vec!["pantalla", "hora"],
        ),
        (
            "Abre los recordatorios y dime la hora",
            vec!["sección", "hora"],
        ),
        (
            "Resume mi documento de bases de datos y dime la hora",
            vec!["documento", "hora"],
        ),
        (
            "¿Qué hora es, cuánta batería tengo, cuánta RAM uso y qué red wifi tengo?",
            vec!["hora", "batería", "ram", "wifi"],
        ),
    ] {
        assert_eq!(compound_kinds(text), Some(expected), "{text}");
    }
}

#[test]
fn a_single_request_is_never_split() {
    for text in [
        "¿Qué hora es?",
        "Recuérdame comprar pan y leche mañana a las 8",
        "Recuérdame revisar la batería y el clima mañana",
        "clima en Trinidad y Tobago",
        "¿Cuánta batería tengo? gracias",
        "Busca mi informe de clima y batería",
        "dame el clima de la ciudad en la que me encuentro",
    ] {
        assert!(
            !matches!(split_compound(text), Compound::Local { .. }),
            "{text}: {:?}",
            compound_kinds(text)
        );
    }
}

#[test]
fn a_request_with_a_part_the_rules_do_not_know_goes_to_the_ai() {
    for text in [
        "¿Qué hora es y busca mi informe de programación?",
        "Dime la batería y busca mi informe de redes",
        // Una parte elíptica necesita el contexto de la anterior: la entiende la IA.
        "¿Cómo está el clima en Medellín? ¿Y en Cali?",
    ] {
        assert_eq!(split_compound(text), Compound::NeedsAi, "{text}");
    }
}

#[test]
fn more_than_four_requests_keep_the_first_four() {
    let Compound::Local { actions, extra } =
        split_compound("¿Qué hora es? ¿Batería? ¿RAM? ¿CPU? ¿Internet? ¿Modo oscuro?")
    else {
        panic!()
    };
    assert_eq!(actions.len(), MAX_PARTS);
    assert_eq!(extra, 2);
}
