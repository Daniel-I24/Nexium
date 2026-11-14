import type { ReactNode, Ref } from "react";

interface ScreenLayoutProps {
  children: ReactNode;
  /** Acciones que deben verse siempre, como Guardar: quedan fuera de la zona que se desplaza. */
  footer?: ReactNode;
  /** La zona que se desplaza, para quien necesite llevar algo a la vista. */
  ref?: Ref<HTMLDivElement>;
}

/**
 * Estructura común de cada pantalla: una zona de contenido con desplazamiento propio y, si hace falta,
 * un pie fijo. La ventana nunca se desplaza: solo esta zona.
 */
export function ScreenLayout({ children, footer, ref }: ScreenLayoutProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={ref} data-scroll-area className="scrollbar-stable min-h-0 flex-1 overflow-y-auto px-4 py-6 short:py-3 sm:px-6 sm:py-8 sm:short:py-3 wide:px-12">
        {/* Contenedor común: centrado, con el mismo ancho máximo y el mismo margen en todas las pantallas. */}
        <div className="mx-auto w-full max-w-content">{children}</div>
      </div>
      {footer && (
        <div className="shrink-0 border-t border-line bg-panel px-4 py-3 short:py-2 sm:px-6 wide:px-12">
          <div className="mx-auto flex w-full max-w-content flex-wrap items-center gap-x-4 gap-y-2">{footer}</div>
        </div>
      )}
    </div>
  );
}
