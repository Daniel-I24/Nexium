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

export function Rail({ active, sensing, onSelect }: RailProps) {
  return (
    <nav
      aria-label="Secciones"
      className="flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 border-b border-hairline px-4 py-3 wide:w-rail wide:flex-col wide:flex-nowrap wide:items-stretch wide:gap-8 wide:border-r wide:border-b-0 wide:py-6"
    >
      <div className="flex items-center gap-3 px-2">
        <NexiumMark sensing={sensing} />
        <span className="text-lg font-semibold">Nexium</span>
      </div>
      <div className="flex flex-wrap gap-1 wide:flex-col">
        {items.map((item) => {
          const selected = item.id === active;
          return (
            <button
              key={item.id}
              type="button"
              aria-current={selected ? "page" : undefined}
              onClick={() => onSelect(item.id)}
              className={`min-h-11 border-l-2 bg-transparent px-2 text-left transition-colors duration-150 ease-instrument ${
                selected ? "border-ink bg-panel font-semibold text-ink" : "border-transparent text-muted hover:text-ink"
              }`}
            >
              {item.label}
            </button>
          );
        })}
      </div>
    </nav>
  );
}
