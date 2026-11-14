//! Lecturas del equipo: CPU, memoria, batería, red, Wi-Fi y modo oscuro.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::join;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    core::format::{gigabytes, whole},
    models::{Category, DataRow, Intent, Reading, ResponseBody, SystemMetric},
    providers::system::SystemProvider,
};

pub struct SystemHandler {
    pub system: Arc<dyn SystemProvider>,
}

fn unavailable_outcome(label: &str, reason: String) -> Outcome {
    Outcome::done(
        Category::System,
        format!("{label}: no disponible"),
        ResponseBody::Rows {
            rows: vec![DataRow::unavailable(label)],
            note: Some(reason),
            attribution: None,
        },
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
                let mut rows = vec![DataRow::value(
                    "Estado de Wi-Fi",
                    yes_no(info.connected, "Conectado", "Sin conexión"),
                )];
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
                Outcome::done(
                    Category::System,
                    summary,
                    ResponseBody::Rows {
                        rows,
                        note,
                        attribution: None,
                    },
                )
            }
            Reading::Unavailable(reason) => unavailable_outcome("Estado de Wi-Fi", reason),
        }
    }

    async fn overview(&self) -> Outcome {
        let (cpu, memory, battery, internet, wifi, dark) = join!(
            self.system.cpu_percent(),
            self.system.memory(),
            self.system.battery(),
            self.system.internet(),
            self.system.wifi(),
            self.system.dark_mode()
        );
        let rows = vec![
            row_for("CPU en uso", cpu, |value| format!("{} %", whole(value))),
            row_for("RAM en uso", memory.clone(), |value| {
                format!(
                    "{} GB de {} GB",
                    gigabytes(value.used_bytes),
                    gigabytes(value.total_bytes)
                )
            }),
            row_for("RAM disponible", memory, |value| {
                format!("{} GB", gigabytes(value.available_bytes))
            }),
            row_for("Batería", battery, |value| {
                format!("{} % · {}", whole(value.percent), value.state)
            }),
            row_for("Internet", internet, |value| {
                yes_no(value, "Conectado", "Sin conexión")
            }),
            row_for("Wi-Fi", wifi, |value| {
                yes_no(value.connected, "Conectado", "Sin conexión")
            }),
            row_for("Modo oscuro", dark, |value| {
                yes_no(value, "Activado", "Desactivado")
            }),
        ];
        Outcome::done(
            Category::System,
            "Estado del equipo",
            ResponseBody::Rows {
                rows,
                note: None,
                attribution: None,
            },
        )
    }
}

impl SystemHandler {
    async fn answer(&self, metric: &SystemMetric) -> Outcome {
        match metric {
            SystemMetric::Cpu => match self.system.cpu_percent().await {
                Reading::Value(percent) => Outcome::done(
                    Category::System,
                    format!("CPU al {} %", whole(percent)),
                    ResponseBody::Reading {
                        value: whole(percent),
                        unit: "%".into(),
                        context: "de CPU en uso".into(),
                        percent: Some(percent),
                        rows: vec![],
                        attribution: None,
                    },
                ),
                Reading::Unavailable(reason) => unavailable_outcome("CPU en uso", reason),
            },
            SystemMetric::Ram => match self.system.memory().await {
                Reading::Value(memory) => {
                    let percent =
                        memory.used_bytes as f64 / memory.total_bytes.max(1) as f64 * 100.0;
                    Outcome::done(
                        Category::System,
                        format!("{} GB de RAM en uso", gigabytes(memory.used_bytes)),
                        ResponseBody::Reading {
                            value: gigabytes(memory.used_bytes),
                            unit: "GB".into(),
                            context: format!("en uso de {} GB", gigabytes(memory.total_bytes)),
                            percent: Some(percent),
                            rows: vec![DataRow::value(
                                "Disponible",
                                format!("{} GB", gigabytes(memory.available_bytes)),
                            )],
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
                    let status =
                        yes_no(connected, "Conectado a Internet", "Sin conexión a Internet");
                    Outcome::done(
                        Category::System,
                        status.clone(),
                        ResponseBody::Rows {
                            rows: vec![DataRow::text("Internet", status)],
                            note: None,
                            attribution: None,
                        },
                    )
                }
                Reading::Unavailable(reason) => unavailable_outcome("Internet", reason),
            },
            SystemMetric::Wifi => self.wifi().await,
            SystemMetric::DarkMode => match self.system.dark_mode().await {
                Reading::Value(dark) => {
                    let status = yes_no(dark, "Activado", "Desactivado");
                    Outcome::done(
                        Category::System,
                        format!("Modo oscuro: {status}"),
                        ResponseBody::Rows {
                            rows: vec![DataRow::text("Modo oscuro", status)],
                            note: None,
                            attribution: None,
                        },
                    )
                }
                Reading::Unavailable(reason) => unavailable_outcome("Modo oscuro", reason),
            },
            SystemMetric::Overview => self.overview().await,
        }
    }
}

#[async_trait]
impl CommandHandler for SystemHandler {
    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Option<Outcome> {
        let Intent::System(metric) = intent else {
            return None;
        };
        Some(self.answer(metric).await)
    }
}
