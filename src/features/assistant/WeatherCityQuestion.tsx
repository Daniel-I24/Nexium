import { useState, type FormEvent } from "react";

import { Button } from "@/components/Button";
import { Notice } from "@/components/Notice";
import { api, errorMessage } from "@/services/api";
import type { WeatherWhen } from "@/types/api";

/** La solicitud que se envía con la ciudad, conservando para cuándo era. */
export function weatherRequest(city: string, when: WeatherWhen): string {
  if (when === "today") return `¿Cómo estará el clima hoy en ${city}?`;
  if (when === "tomorrow") return `¿Cómo estará el clima mañana en ${city}?`;
  return `¿Cómo está el clima en ${city}?`;
}

/**
 * Falta la ciudad y no hay una en Ajustes: se pregunta aquí mismo y se ofrece guardarla. Si se guarda, se
 * comprueba antes con Open-Meteo y se usa su nombre oficial.
 */
export function WeatherCityQuestion({ when, onAsk }: { when: WeatherWhen; onAsk: (request: string) => void }) {
  const [city, setCity] = useState("");
  const [save, setSave] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const typed = city.trim();
    if (!typed) return;
    setBusy(true);
    setError(null);
    try {
      const name = save ? await api.setWeatherCity(typed) : typed;
      onAsk(weatherRequest(name, when));
    } catch (reason) {
      setError(errorMessage(reason));
      setBusy(false);
    }
  };

  return (
    <form onSubmit={(event) => void submit(event)} className="flex max-w-reading flex-col gap-3">
      <p className="m-0 text-lg text-query">¿De qué ciudad quieres el clima?</p>
      <label className="flex flex-col gap-1 text-sm text-muted">
        Ciudad
        <input
          autoFocus
          value={city}
          maxLength={100}
          disabled={busy}
          onChange={(event) => setCity(event.target.value)}
          className="h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 text-md text-ink"
        />
      </label>
      <label className="flex min-h-11 items-center gap-3">
        <input type="checkbox" className="size-4 shrink-0 accent-ink" checked={save} disabled={busy} onChange={(event) => setSave(event.target.checked)} />
        <span>Guardarla como ciudad para el clima</span>
      </label>
      {error && <Notice tone="error">{error}</Notice>}
      <div>
        <Button type="submit" variant="primary" disabled={busy || !city.trim()}>
          Consultar el clima
        </Button>
      </div>
    </form>
  );
}
