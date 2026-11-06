const examples = ["¿Cuánta memoria RAM estoy utilizando?", "¿Cómo está el clima hoy?", "Recuérdame entregar el proyecto el viernes a las nueve"];

export function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="flex max-w-reading flex-col gap-4">
      <h1 className="m-0 text-xl font-medium leading-tight">¿Qué necesitas saber de tu equipo?</h1>
      <p className="m-0 text-muted">Pulsa Hablar o escribe tu solicitud. También puedes empezar por uno de estos ejemplos:</p>
      <ul className="m-0 flex list-none flex-col border-b border-hairline p-0">
        {examples.map((example) => (
          <li key={example} className="border-t border-hairline">
            <button type="button" onClick={() => onPick(example)} className="min-h-11 w-full bg-transparent py-2 text-left text-ink hover:text-muted">
              {example}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
