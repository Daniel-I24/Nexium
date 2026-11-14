import { useEffect, useRef, type ReactNode } from "react";

import { Rail, type Screen } from "./Rail";

interface ShellProps {
  screen: Screen;
  sensing: boolean;
  onSelect: (screen: Screen) => void;
  bar: ReactNode;
  children: ReactNode;
}

/**
 * Composición fluida: navegación, área de lectura desplazable y barra de señal siempre visible.
 *
 * - El área principal es la única que se desplaza en uso normal, y nunca baja de 10rem.
 * - La barra no pasa de la mitad del alto: si su contenido no cabe, se desplaza por dentro.
 * - Si aun así no cabe todo (ventana mínima con el texto muy ampliado), se desplaza la ventana entera:
 *   nada queda cortado ni fuera de alcance.
 */
export function Shell({ screen, sensing, onSelect, bar, children }: ShellProps) {
  const main = useRef<HTMLElement>(null);

  // Cada pantalla empieza arriba: el área principal es la misma y conservaría el desplazamiento de la anterior.
  useEffect(() => {
    main.current?.scrollTo({ top: 0 });
  }, [screen]);

  return (
    <div className="flex h-dvh flex-col overflow-y-auto bg-base text-ink wide:flex-row">
      <Rail active={screen} sensing={sensing} onSelect={onSelect} />
      <div className="grid min-w-0 flex-1 grid-rows-shell bg-panel">
        <main ref={main} className="scrollbar-stable min-h-0 scroll-pb-32 overflow-y-auto px-4 py-6 sm:px-6 sm:py-8 wide:px-12">
          {/* Contenedor común de todas las pantallas: centrado, con el mismo ancho máximo y el mismo margen. */}
          <div key={screen} className="mx-auto w-full max-w-content animate-section">
            {children}
          </div>
        </main>
        {bar}
      </div>
    </div>
  );
}
