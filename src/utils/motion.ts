import { flushSync } from "react-dom";

/** Con «Reducir movimiento» (o sin forma de saberlo, como en las pruebas), los cambios son instantáneos. */
export function prefersReducedMotion(): boolean {
  return typeof window.matchMedia !== "function" || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * Aplica un cambio de pantalla con una transición de vista: la vista anterior sale mientras entra la nueva,
 * sin un instante en blanco. Si el motor no las admite o se pidió reducir movimiento, el cambio es directo.
 */
export function withViewTransition(update: () => void): void {
  if (prefersReducedMotion() || typeof document.startViewTransition !== "function") {
    update();
    return;
  }
  document.startViewTransition(() => flushSync(update));
}

/** Lo que dura `animate-leave` (duration-fast), con un margen para que el último fotograma llegue a verse. */
const LEAVE_MS = 170;

/**
 * Anima la salida de algo y, al terminar, ejecuta la acción que lo quita. Con movimiento reducido,
 * la acción es inmediata.
 */
export function exitThen(markLeaving: () => void, action: () => void): void {
  if (prefersReducedMotion()) {
    action();
    return;
  }
  markLeaving();
  window.setTimeout(action, LEAVE_MS);
}

/** Retraso escalonado de la entrada de una lista: 30 ms por elemento, solo en los seis primeros. */
export function stagger(index: number): { animationDelay: string } {
  return { animationDelay: `${Math.min(index, 5) * 30}ms` };
}
