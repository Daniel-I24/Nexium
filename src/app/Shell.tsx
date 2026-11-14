import type { ReactNode } from "react";

import { Rail, type Screen } from "./Rail";

interface ShellProps {
  screen: Screen;
  sensing: boolean;
  onSelect: (screen: Screen) => void;
  bar: ReactNode;
  children: ReactNode;
}

/**
 * Marco de la app: ocupa exactamente la ventana y nunca se desplaza.
 *
 * - La navegación es una columna fija a la izquierda.
 * - Cada pantalla tiene su propia zona de desplazamiento (ScreenLayout) y, si la necesita, un pie fijo.
 * - La barra de señal queda abajo, siempre entera: se compacta según su ancho, nunca se corta.
 */
export function Shell({ screen, sensing, onSelect, bar, children }: ShellProps) {
  return (
    <div data-shell className="flex h-dvh overflow-hidden bg-base text-ink">
      <Rail active={screen} sensing={sensing} onSelect={onSelect} />
      <div className="flex min-w-0 flex-1 flex-col bg-panel">
        <main key={screen} className="view-section flex min-h-0 flex-1 flex-col">
          {children}
        </main>
        {bar}
      </div>
    </div>
  );
}
