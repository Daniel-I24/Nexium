const inputStyle = "h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 font-mono text-md text-ink";

/**
 * Fecha y hora de un recordatorio en dos campos. Un único campo de fecha y hora no cabe en una ventana
 * estrecha con el texto ampliado; dos campos se colocan uno junto a otro o uno bajo otro.
 * El valor es `AAAA-MM-DDTHH:MM` en hora local, completo o no.
 */
export function DueFields({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const [date = "", time = ""] = value.split("T");
  return (
    <div className="flex flex-wrap gap-3">
      <label className="flex min-w-0 flex-1 basis-40 flex-col gap-1 text-sm text-muted">
        Fecha
        <input type="date" value={date} onChange={(event) => onChange(`${event.target.value}T${time}`)} className={inputStyle} />
      </label>
      <label className="flex min-w-0 flex-1 basis-32 flex-col gap-1 text-sm text-muted">
        Hora
        <input type="time" value={time} onChange={(event) => onChange(`${date}T${event.target.value}`)} className={inputStyle} />
      </label>
    </div>
  );
}
