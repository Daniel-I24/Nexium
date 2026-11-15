import { Icon } from "@/components/Icon";
import { NexiumMark } from "@/components/NexiumMark";
import type { Section } from "@/types/api";

export type Screen = Section;

export const items: { id: Screen; label: string }[] = [
  { id: "assistant", label: "Asistente" },
  { id: "reminders", label: "Recordatorios" },
  { id: "history", label: "Historial" },
  { id: "settings", label: "Ajustes" },
];

interface RailProps {
  active: Screen;
  sensing: boolean;
  onSelect: (screen: Screen) => void;
}

/**
 * Navegación fija a la izquierda, siempre en columna: el alto es lo que más escasea en una ventana de escritorio.
 * En ventanas anchas muestra icono y nombre; en las estrechas, solo el icono, con el nombre como etiqueta accesible
 * y como sugerencia al pasar el ratón. En ventanas muy bajas se oculta la marca para que quepan las cuatro secciones.
 */
export function Rail({ active, sensing, onSelect }: RailProps) {
  return (
    <nav
      aria-label="Secciones"
      className="flex w-rail-compact shrink-0 flex-col gap-4 overflow-y-auto border-r border-hairline py-3 short:py-1 wide:w-rail wide:gap-8 wide:py-6"
    >
      <div className="flex items-center justify-center gap-3 short:hidden wide:justify-start wide:px-4">
        <NexiumMark sensing={sensing} />
        <span className="sr-only text-lg font-semibold wide:not-sr-only">Nexium</span>
      </div>
      <ul className="m-0 flex list-none flex-col gap-1 p-0">
        {items.map((item) => {
          const selected = item.id === active;
          return (
            <li key={item.id}>
              <button
                type="button"
                aria-current={selected ? "page" : undefined}
                title={item.label}
                onClick={() => onSelect(item.id)}
                className={`flex min-h-11 w-full cursor-pointer items-center justify-center gap-3 border-l-2 bg-transparent px-2 text-left wide:justify-start wide:px-4 ${
                  selected ? "border-ink bg-panel font-semibold text-ink" : "border-transparent text-muted hover:text-ink"
                }`}
              >
                <Icon name={item.id} size="md" />
                <span className="sr-only wide:not-sr-only">{item.label}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
