import type { ReactNode } from "react";

import { Rail, type Screen } from "./Rail";

interface ShellProps {
  screen: Screen;
  sensing: boolean;
  onSelect: (screen: Screen) => void;
  bar: ReactNode;
  children: ReactNode;
}

/** Composición fija: navegación, área de lectura desplazable y barra de señal siempre visible. */
export function Shell({ screen, sensing, onSelect, bar, children }: ShellProps) {
  return (
    <div className="flex h-dvh flex-col bg-base text-ink wide:flex-row">
      <Rail active={screen} sensing={sensing} onSelect={onSelect} />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-panel">
        {/* Contenedor común de todas las pantallas: centrado, con el mismo ancho máximo y el mismo margen. */}
        <main className="scrollbar-stable min-h-0 flex-1 overflow-y-auto px-6 py-8 wide:px-12">
          <div className="mx-auto w-full max-w-content">{children}</div>
        </main>
        {bar}
      </div>
    </div>
  );
}
