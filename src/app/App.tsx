import { useEffect, useState } from "react";

import { AssistantScreen } from "@/features/assistant/AssistantScreen";
import { useAssistant } from "@/features/assistant/useAssistant";
import { HistoryScreen } from "@/features/history/HistoryScreen";
import { RemindersScreen } from "@/features/reminders/RemindersScreen";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { isSensing } from "@/features/voice/barState";
import { SignalBar } from "@/features/voice/SignalBar";
import { events } from "@/services/api";
import { withViewTransition } from "@/utils/motion";

import type { Screen } from "./Rail";
import { Shell } from "./Shell";

export function App() {
  const [screen, setScreenNow] = useState<Screen>("assistant");
  const [highlight, setHighlight] = useState<number | null>(null);

  /** El cambio de sección se anima: la anterior sale mientras entra la nueva. */
  const setScreen = (next: Screen) => {
    if (next === screen) return;
    // El resaltado es de una visita: al volver a Recordatorios ya no se repite.
    setHighlight(null);
    withViewTransition(() => setScreenNow(next));
  };

  const assistant = useAssistant({ onNavigate: setScreen });

  // «Abrir Nexium» en el aviso de un recordatorio: Recordatorios, con ese recordatorio resaltado.
  useEffect(() => {
    const subscription = events.onOpenReminder((id) => {
      setHighlight(id);
      withViewTransition(() => setScreenNow("reminders"));
    });
    return () => void subscription.then((unlisten) => unlisten());
  }, []);

  const goToAssistant = (action: () => void) => {
    setScreen("assistant");
    action();
  };

  return (
    <Shell
      screen={screen}
      sensing={isSensing(assistant.bar)}
      onSelect={setScreen}
      bar={
        <SignalBar
          state={assistant.bar}
          levels={assistant.levels}
          onSpeak={() => goToAssistant(() => void assistant.speak())}
          onStopListening={() => void assistant.stopListening()}
          onCancel={() => void assistant.cancel()}
          onSubmitText={(text) => goToAssistant(() => void assistant.submit(text))}
          onAnalyzeScreen={() => goToAssistant(assistant.analyzeScreenButton)}
          onConfirmCapture={() => void assistant.confirmCapture()}
          onRetry={assistant.retry}
        />
      }
    >
      {screen === "assistant" && (
        <AssistantScreen
          current={assistant.current}
          tape={assistant.tape}
          onPickExample={(text) => void assistant.submit(text)}
          onSaveReminder={(content, dueAt) => void assistant.saveReminder(content, dueAt)}
          onAuthorizeFolders={(choice, pendingRequest) => void assistant.authorizeFolders(choice, pendingRequest)}
          onAnalyzeDocument={(candidate, task) => void assistant.analyzeDocument(candidate, task)}
          onDismiss={assistant.dismissCurrent}
          onChooseReminderOption={assistant.chooseReminderOption}
          onChooseSuggestion={assistant.chooseSuggestion}
        />
      )}
      {screen === "reminders" && <RemindersScreen highlight={highlight} onPickExample={(text) => goToAssistant(() => void assistant.submit(text))} />}
      {screen === "history" && <HistoryScreen onCleared={assistant.clearConversation} />}
      {screen === "settings" && <SettingsScreen />}
    </Shell>
  );
}
