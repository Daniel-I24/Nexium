//! Hora y fecha del reloj del equipo.

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    core::format::{clock, long_date},
    models::{Category, Intent, ResponseBody},
};

pub struct TimeHandler;

impl TimeHandler {
    fn answer(context: &HandlerContext) -> Outcome {
        let time = clock(&context.now);
        let date = long_date(context.now.date_naive());
        Outcome::done(
            Category::Time,
            format!("{time} · {date}"),
            ResponseBody::Reading {
                value: time,
                unit: String::new(),
                context: date,
                percent: None,
                rows: vec![],
                attribution: None,
            },
        )
    }
}

#[async_trait]
impl CommandHandler for TimeHandler {
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Option<Outcome> {
        matches!(intent, Intent::Time).then(|| Self::answer(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::handlers::test_support::now;
    use chrono::Duration;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn the_time_comes_from_the_clock() {
        let context = HandlerContext {
            request: "hora".into(),
            now: now() + Duration::hours(4),
            cancel: CancellationToken::new(),
        };
        let outcome = TimeHandler.handle(&Intent::Time, &context).await.unwrap();
        assert_eq!(outcome.summary, "14:00 · jueves 5 de noviembre");
    }
}
