import { NexiumMark } from "@/components/NexiumMark";

export type Screen = "assistant" | "reminders" | "history" | "settings";

const items: { id: Screen; label: string }[] = [
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

/** En ventanas estrechas es una franja superior y la sección activa se subraya; en anchas, una columna con marca a la izquierda. */
export function Rail({ active, sensing, onSelect }: RailProps) {
  return (
    <nav
      aria-label="Secciones"
      className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b border-hairline px-2 py-1 sm:px-4 wide:w-rail wide:flex-col wide:flex-nowrap wide:items-stretch wide:gap-8 wide:overflow-y-auto wide:border-r wide:border-b-0 wide:py-6"
    >
      <div className="flex items-center gap-3 px-2">
        <NexiumMark sensing={sensing} />
        <span className="text-lg font-semibold">Nexium</span>
      </div>
      <ul className="m-0 flex list-none flex-wrap gap-1 p-0 wide:flex-col">
        {items.map((item) => {
          const selected = item.id === active;
          return (
            <li key={item.id}>
              <button
                type="button"
                aria-current={selected ? "page" : undefined}
                onClick={() => onSelect(item.id)}
                className={`min-h-11 w-full border-b-2 bg-transparent px-2 text-left wide:border-b-0 wide:border-l-2 ${
                  selected ? "border-ink font-semibold text-ink wide:bg-panel" : "border-transparent text-muted hover:text-ink"
                }`}
              >
                {item.label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
