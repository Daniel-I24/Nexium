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
  return (
    <div className="flex flex-col gap-10">
      <Tape entries={tape} />
      {current ? (
        <ResponseView key={current.id} response={current} actions={{ onSaveReminder, onAuthorizeFolders }} />
      ) : (
        <EmptyState onPick={onPickExample} />
      )}
    </div>
  );
}
