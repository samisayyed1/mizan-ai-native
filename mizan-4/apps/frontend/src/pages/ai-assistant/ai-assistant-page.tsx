import { ChatShell } from "@/features/ai-assistant";
import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";

/**
 * AI Assistant page - main chat interface for Mizan AI.
 */
export default function AiAssistantPage() {
  const [searchParams] = useSearchParams();
  const intent = searchParams.get("intent");
  const prompt = searchParams.get("prompt");
  const initialIntent = useMemo(() => {
    if (!intent) return undefined;
    return {
      intent,
      prompt:
        prompt ??
        "Tell me what you want to add — for example: 15.5 oz gold, a rental property in Toronto, my RRSP statement, or a CSV from my broker.",
    };
  }, [intent, prompt]);

  return <ChatShell className="h-full min-h-0" initialIntent={initialIntent} />;
}
