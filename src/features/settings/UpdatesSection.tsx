import { useState } from "react";

import { Button } from "@/components/Button";
import { Notice } from "@/components/Notice";
import { ProgressLine } from "@/components/Progress";
import { api, errorMessage } from "@/services/api";

type State =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate"; current: string }
  | { kind: "unpublished"; current: string }
  | { kind: "available"; current: string; available: string }
  | { kind: "installing"; available: string }
  | { kind: "error"; message: string };

/**
 * Actualizaciones desde GitHub Releases. Nexium solo comprueba cuando se pide, e instala únicamente versiones
 * firmadas con su clave; al terminar se reinicia.
 */
export function UpdatesSection() {
  const [state, setState] = useState<State>({ kind: "idle" });

  const check = async () => {
    setState({ kind: "checking" });
    try {
      const status = await api.checkUpdate();
      if (status.available) setState({ kind: "available", current: status.current, available: status.available });
      else setState({ kind: status.published ? "upToDate" : "unpublished", current: status.current });
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  };

  const install = async (available: string) => {
    setState({ kind: "installing", available });
    try {
      await api.installUpdate();
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  };

  return (
    <section className="flex max-w-reading flex-col gap-3 border-t border-line pt-4">
      <h2 className="m-0 text-lg font-medium">Actualizaciones</h2>
      <p className="m-0 text-sm text-muted">Nexium solo busca versiones nuevas cuando lo pides, en sus publicaciones de GitHub, y solo instala las firmadas por Nexium.</p>
      {state.kind === "upToDate" && <Notice tone="ok">{`Tienes la última versión, la ${state.current}.`}</Notice>}
      {state.kind === "unpublished" && (
        <p role="status" className="m-0">
          Todavía no hay ninguna versión publicada en GitHub. Tienes la {state.current}.
        </p>
      )}
      {state.kind === "available" && (
        <p role="status" className="m-0">
          Hay una versión nueva: la {state.available}. Tienes la {state.current}. Al instalarla, Nexium se cerrará y se volverá a abrir.
        </p>
      )}
      {state.kind === "installing" && (
        <div role="status" className="flex flex-col gap-2 text-muted">
          <span>Descargando e instalando la versión {state.available}…</span>
          <ProgressLine className="w-40 max-w-full" />
        </div>
      )}
      {state.kind === "error" && <Notice tone="error">{state.message}</Notice>}
      <div className="flex flex-wrap gap-2">
        {state.kind === "available" ? (
          <Button variant="primary" onClick={() => void install(state.available)}>
            Instalar la versión {state.available}
          </Button>
        ) : (
          <Button disabled={state.kind === "checking" || state.kind === "installing"} onClick={() => void check()}>
            {state.kind === "checking" ? "Buscando…" : "Buscar actualizaciones"}
          </Button>
        )}
      </div>
    </section>
  );
}
