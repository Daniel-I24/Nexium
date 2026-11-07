import { useEffect, useRef } from "react";

import type { AssistantResponse } from "@/types/api";

import { EmptyState } from "./EmptyState";
import { ResponseView, type ResponseActions } from "./ResponseView";
import { Tape } from "./Tape";

interface AssistantScreenProps extends ResponseActions {
  current: AssistantResponse | null;
  tape: AssistantResponse[];
  onPickExample: (text: string) => void;
}

export function AssistantScreen({ current, tape, onPickExample, onSaveReminder, onAuthorizeFolders }: AssistantScreenProps) {
  const responseStart = useRef<HTMLDivElement>(null);
  const currentId = current?.id;

  // La cinta crece por encima: sin esto, la respuesta nueva queda fuera de la vista.
  useEffect(() => {
    if (currentId !== undefined) responseStart.current?.scrollIntoView({ block: "start" });
  }, [currentId]);

  return (
    <div className="flex flex-col gap-10">
      <Tape entries={tape} />
      {current ? (
        <div ref={responseStart}>
          <ResponseView key={current.id} response={current} actions={{ onSaveReminder, onAuthorizeFolders }} />
        </div>
      ) : (
        <EmptyState onPick={onPickExample} />
      )}
    </div>
  );
}
