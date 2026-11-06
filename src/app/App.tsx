import { useState } from "react";

import { AssistantScreen } from "@/features/assistant/AssistantScreen";
import { useAssistant } from "@/features/assistant/useAssistant";
import { HistoryScreen } from "@/features/history/HistoryScreen";
import { RemindersScreen } from "@/features/reminders/RemindersScreen";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { isSensing } from "@/features/voice/barState";
import { SignalBar } from "@/features/voice/SignalBar";

import type { Screen } from "./Rail";
import { Shell } from "./Shell";

export function App() {
  const [screen, setScreen] = useState<Screen>("assistant");
  const assistant = useAssistant();

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
          onAnalyzeScreen={() => goToAssistant(() => void assistant.requestCapture())}
          onConfirmCapture={() => void assistant.confirmCapture()}
          onRetry={assistant.retry}
          onChooseReminderOption={assistant.chooseReminderOption}
          onChooseSuggestion={assistant.chooseSuggestion}
        />
      }
    >
      {screen === "assistant" && (
        <AssistantScreen
          current={assistant.current}
          tape={assistant.tape}
          onPickExample={(text) => void assistant.submit(text)}
          onSaveReminder={(content, dueAt) => void assistant.saveReminder(content, dueAt)}
          onAuthorizeFolders={(folders, pendingRequest) => void assistant.authorizeFolders(folders, pendingRequest)}
        />
      )}
      {screen === "reminders" && <RemindersScreen />}
      {screen === "history" && <HistoryScreen />}
      {screen === "settings" && <SettingsScreen />}
    </Shell>
  );
}
