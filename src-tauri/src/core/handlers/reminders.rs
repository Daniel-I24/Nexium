//! Borradores de recordatorio y lista de los pendientes. Nada se guarda sin confirmación.

use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    core::format::{clock, long_date},
    error::AppError,
    models::{Category, DataRow, Intent, ReminderOption, ReminderRequest, ResponseBody},
    services::reminders::ReminderService,
};

pub struct ReminderIntentHandler {
    pub reminders: ReminderService,
}

fn local_iso(date: NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
    Local.from_local_datetime(&date.and_time(time)).earliest()
}

/// Convierte lo interpretado en un borrador. Nada se guarda hasta que el usuario confirma.
pub fn draft_reminder(request: &ReminderRequest, now: DateTime<Local>) -> ResponseBody {
    let content = request.content.clone();
    let due = request
        .date
        .zip(request.time)
        .and_then(|(date, time)| local_iso(date, time));
    if let Some(question) = &request.ambiguity {
        return ResponseBody::ReminderDraft {
            content,
            due_at: due
                .filter(|value| *value > now)
                .map(|value| value.to_rfc3339()),
            question: Some(question.clone()),
            options: vec![],
        };
    }
    match (request.date, request.time, due) {
        (_, _, Some(value)) if value <= now => ResponseBody::ReminderDraft {
            content,
            due_at: None,
            question: Some(format!(
                "El {} a las {} ya pasó. ¿Para cuándo lo quieres?",
                long_date(value.date_naive()),
                clock(&value)
            )),
            options: vec![],
        },
        (_, _, Some(value)) => ResponseBody::ReminderDraft {
            content,
            due_at: Some(value.to_rfc3339()),
            question: None,
            options: vec![],
        },
        (Some(date), None, _) => {
            let options = [(8, "08:00"), (12, "12:00"), (18, "18:00")]
                .iter()
                .filter_map(|(hour, label)| {
                    let at = local_iso(date, NaiveTime::from_hms_opt(*hour, 0, 0)?)?;
                    (at > now).then(|| ReminderOption {
                        label: (*label).to_owned(),
                        due_at: at.to_rfc3339(),
                    })
                })
                .collect();
            ResponseBody::ReminderDraft {
                content,
                due_at: None,
                question: Some(format!(
                    "¿A qué hora del {} quieres el aviso?",
                    long_date(date)
                )),
                options,
            }
        }
        _ => ResponseBody::ReminderDraft {
            content,
            due_at: None,
            question: Some("¿Qué día y a qué hora quieres el aviso?".into()),
            options: vec![],
        },
    }
}

#[async_trait]
impl CommandHandler for ReminderIntentHandler {
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Option<Outcome> {
        match intent {
            Intent::CreateReminder(request) => {
                let body = draft_reminder(request, context.now);
                let summary = if let ResponseBody::ReminderDraft {
                    question: Some(question),
                    ..
                } = &body
                {
                    question.clone()
                } else {
                    format!("Por confirmar: {}", request.content)
                };
                Some(Outcome::pending(Category::Reminders, summary, body))
            }
            Intent::ListReminders => Some(match self.reminders.list() {
                Ok(items) if items.is_empty() => Outcome::done(
                    Category::Reminders,
                    "Sin recordatorios",
                    ResponseBody::Text {
                        text: "No tienes recordatorios pendientes.".into(),
                    },
                ),
                Ok(items) => {
                    let count = items.len();
                    let rows = items
                        .into_iter()
                        .map(|item| {
                            let due = DateTime::parse_from_rfc3339(&item.due_at).map_or_else(
                                |_| item.due_at.clone(),
                                |date| {
                                    let local = date.with_timezone(&Local);
                                    format!("{} · {}", long_date(local.date_naive()), clock(&local))
                                },
                            );
                            DataRow::value(&item.content, due)
                        })
                        .collect();
                    Outcome::done(
                        Category::Reminders,
                        crate::core::format::count(count, "recordatorio", "recordatorios"),
                        ResponseBody::Rows {
                            rows,
                            note: None,
                            attribution: None,
                        },
                    )
                }
                Err(AppError::Database(_) | AppError::Operation(_) | AppError::Validation(_)) => {
                    Outcome::failed(
                        Category::Reminders,
                        "database",
                        "No pude leer tus recordatorios.",
                        None,
                    )
                }
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::handlers::test_support::now;
    use chrono::Duration;
    use tokio_util::sync::CancellationToken;

    fn request(
        date: Option<(i32, u32, u32)>,
        time: Option<(u32, u32)>,
        ambiguity: Option<&str>,
    ) -> ReminderRequest {
        ReminderRequest {
            content: "Entregar".into(),
            date: date.and_then(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d)),
            time: time.and_then(|(h, m)| NaiveTime::from_hms_opt(h, m, 0)),
            ambiguity: ambiguity.map(str::to_owned),
        }
    }

    #[test]
    fn a_complete_date_only_needs_confirmation() {
        let ResponseBody::ReminderDraft {
            due_at, question, ..
        } = draft_reminder(&request(Some((2026, 11, 12)), Some((8, 0)), None), now())
        else {
            panic!()
        };
        assert!(due_at.is_some());
        assert_eq!(question, None);
    }

    #[test]
    fn a_missing_hour_offers_options() {
        let ResponseBody::ReminderDraft {
            due_at,
            question,
            options,
            ..
        } = draft_reminder(&request(Some((2026, 11, 12)), None, None), now())
        else {
            panic!()
        };
        assert_eq!(due_at, None);
        assert!(question.unwrap().contains("jueves 12 de noviembre"));
        assert_eq!(options.len(), 3);
    }

    #[test]
    fn options_already_past_are_not_offered() {
        let ResponseBody::ReminderDraft { options, .. } =
            draft_reminder(&request(Some((2026, 11, 5)), None, None), now())
        else {
            panic!()
        };
        assert_eq!(
            options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(),
            ["12:00", "18:00"]
        );
    }

    #[test]
    fn a_missing_date_is_asked_for() {
        let ResponseBody::ReminderDraft { question, .. } =
            draft_reminder(&request(None, None, None), now())
        else {
            panic!()
        };
        assert!(question.is_some());
    }

    #[test]
    fn a_past_date_is_not_accepted() {
        let ResponseBody::ReminderDraft {
            due_at, question, ..
        } = draft_reminder(&request(Some((2026, 11, 4)), Some((8, 0)), None), now())
        else {
            panic!()
        };
        assert_eq!(due_at, None);
        assert!(question.unwrap().contains("ya pasó"));
    }

    #[test]
    fn the_model_ambiguity_is_passed_on() {
        let ResponseBody::ReminderDraft { question, .. } = draft_reminder(
            &request(
                Some((2026, 11, 12)),
                Some((8, 0)),
                Some("¿A las 8 de la mañana o de la noche?"),
            ),
            now(),
        ) else {
            panic!()
        };
        assert_eq!(
            question.as_deref(),
            Some("¿A las 8 de la mañana o de la noche?")
        );
    }

    #[test]
    fn an_ambiguous_past_date_is_not_proposed() {
        let ResponseBody::ReminderDraft {
            due_at, question, ..
        } = draft_reminder(
            &request(
                Some((2020, 6, 5)),
                Some((10, 0)),
                Some("¿De la mañana o de la noche?"),
            ),
            now(),
        )
        else {
            panic!()
        };
        assert_eq!(due_at, None);
        assert!(question.is_some());
    }

    #[tokio::test]
    async fn the_reminder_count_agrees_with_the_noun() {
        let reminders = crate::services::reminders::ReminderService::new(
            crate::database::Database::in_memory().unwrap(),
        );
        let handler = ReminderIntentHandler {
            reminders: reminders.clone(),
        };
        let context = HandlerContext {
            request: "mis recordatorios".into(),
            now: now(),
            cancel: CancellationToken::new(),
        };
        let soon = (chrono::Utc::now() + Duration::days(1)).to_rfc3339();
        reminders
            .create("Pagar", &soon, chrono::Utc::now())
            .unwrap();
        let one = handler
            .handle(&Intent::ListReminders, &context)
            .await
            .unwrap();
        assert_eq!(one.summary, "1 recordatorio");
        reminders
            .create("Llamar", &soon, chrono::Utc::now())
            .unwrap();
        let two = handler
            .handle(&Intent::ListReminders, &context)
            .await
            .unwrap();
        assert_eq!(two.summary, "2 recordatorios");
    }
}
