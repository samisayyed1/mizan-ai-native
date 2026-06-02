// Web adapter - AI Chat Streaming (platform-specific HTTP implementation)

import { logger, AI_CHAT_STREAM_ENDPOINT } from "./core";
import type { AiSendMessageRequest, AiStreamEvent } from "@/features/ai-assistant/types";

/**
 * Request shape for the autonomous agent runtime, mirroring the Rust
 * `AgentRunRequest`. Kept here so the web adapter exposes the same
 * type surface as the Tauri adapter — anything that imports it from
 * `@/adapters` works under both build targets even though the agent
 * runtime itself is desktop-only.
 */
export interface AgentRunRequest {
  content: string;
  attachments?: AiSendMessageRequest["attachments"];
  threadId?: string;
  recipeId?: string;
}

/**
 * Stream AI chat responses via HTTP fetch.
 *
 * Uses NDJSON streaming for efficient event delivery.
 *
 * @param request - The chat message request
 * @param signal - Optional AbortSignal for cancellation
 * @yields AiStreamEvent objects from the stream
 */
export async function* streamAiChat(
  request: AiSendMessageRequest,
  signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
  const response = await fetch(AI_CHAT_STREAM_ENDPOINT, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
    signal,
    credentials: "same-origin",
  });

  if (!response.ok) {
    let errorMessage = response.statusText;
    let errorCode = "network";

    try {
      const errorBody = (await response.json()) as { code?: string; error?: string };
      errorCode = errorBody.code ?? "network";
      errorMessage = errorBody.error ?? errorMessage;
    } catch {
      // Ignore JSON parse error
    }

    yield {
      type: "error",
      threadId: "",
      runId: "",
      messageId: undefined,
      code: errorCode,
      message: errorMessage,
    } as AiStreamEvent;
    return;
  }

  if (!response.body) {
    yield {
      type: "error",
      threadId: "",
      runId: "",
      messageId: undefined,
      code: "network",
      message: "Response body is null",
    } as AiStreamEvent;
    return;
  }

  // Parse NDJSON stream
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  try {
    while (true) {
      const { done, value } = await reader.read();

      if (done) {
        // Process any remaining buffer content
        if (buffer.trim()) {
          try {
            const event = JSON.parse(buffer.trim()) as AiStreamEvent;
            yield event;
          } catch (parseError) {
            logger.error("Failed to parse final buffer:", parseError);
          }
        }
        break;
      }

      // Decode chunk and add to buffer
      buffer += decoder.decode(value, { stream: true });

      // Split by newlines and process complete lines
      const lines = buffer.split("\n");

      // Keep the last incomplete line in the buffer
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        try {
          const event = JSON.parse(trimmed) as AiStreamEvent;
          yield event;

          // Stop on terminal events
          if (event.type === "done" || event.type === "error") {
            return;
          }
        } catch (parseError) {
          logger.error("Failed to parse NDJSON line:", trimmed, parseError);
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

/**
 * Stream an autonomous agent run.
 *
 * The agent runtime (Plan → Execute → Verify → Undo) only ships in
 * the Tauri desktop build — it depends on the local Rust agent
 * services and isn't exposed by the web-mode Axum server. The web
 * adapter still has to *export* this function because Vite's alias
 * resolver picks one adapter per build and consumers import from
 * `@/adapters` uniformly. On web we emit a single structured `error`
 * event so any code that reaches it surfaces a clear "desktop only"
 * message instead of a missing-export build failure.
 */
export async function* streamAgentChat(
  _request: AgentRunRequest,
  _signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
  logger.warn("streamAgentChat called in web build — agent runtime is desktop-only.");
  yield {
    type: "error",
    threadId: "",
    runId: "",
    messageId: undefined,
    code: "unsupported_platform",
    message: "The autonomous agent is only available in the Mizan desktop app.",
  } as AiStreamEvent;
}
