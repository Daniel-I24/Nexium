import { Icon } from "./Icon";

/**
 * Resultado de una acción o fallo de una pantalla. Aparece con un movimiento breve para que se note
 * dónde está, y se anuncia a los lectores de pantalla.
 */
export function Notice({ tone, children }: { tone: "ok" | "error"; children: string }) {
  const ok = tone === "ok";
  return (
    <p
      role={ok ? "status" : "alert"}
      className={`m-0 flex animate-enter items-start gap-2 ${ok ? "text-ok" : "text-error"}`}
    >
      <span className="flex h-lh items-center">
        <Icon name={ok ? "completed" : "failed"} />
      </span>
      <span>{children}</span>
    </p>
  );
}
