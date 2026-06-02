/**
 * AI Chat Streaming API client.
 *
 * Re-exports the streaming function from the adapter layer.
 * The adapter handles environment detection (Tauri vs Web) at build time.
 */

import { streamAgentChat, streamAiChat } from "@/adapters";
import type { AiSendMessageRequest, AiStreamEvent, AiChatModelConfig } from "../types";

/**
 * Module-level toggle for Agent Mode. The composer's "Agent" button
 * flips this; useChatRuntime reads it at send time to route between
 * the legacy chat dispatcher and the autonomous agent runtime.
 *
 * A module-level boolean (not a React context) is the simplest channel
 * for a single-thread, single-composer surface — there's exactly one
 * chat shell mounted at a time. Refactor to context if/when we
 * support multiple concurrent chats.
 */
let agentModeEnabled = false;

export function setAgentMode(enabled: boolean): void {
  agentModeEnabled = enabled;
}

export function isAgentMode(): boolean {
  return agentModeEnabled;
}

// Re-export types for convenience
export type { AiSendMessageRequest, AiStreamEvent, AiChatModelConfig };

// Alias for backward compatibility
export type SendMessageRequest = AiSendMessageRequest;
export type ChatModelConfig = AiChatModelConfig;

/**
 * Stream chat response events from the AI assistant.
 *
 * The adapter handles environment detection at build time:
 * - Tauri (desktop): Uses IPC with Channel for streaming
 * - Web: Uses HTTP fetch with NDJSON streaming
 *
 * @param request - The chat message request
 * @param signal - Optional AbortSignal for cancellation
 * @yields AiStreamEvent objects from the stream
 *
 * @example
 * ```ts
 * for await (const event of streamChatResponse({ content: "Show holdings" })) {
 *   if (event.type === "textDelta") {
 *     console.log(event.delta);
 *   } else if (event.type === "done") {
 *     console.log("Complete:", event.message);
 *   }
 * }
 * ```
 */
export async function* streamChatResponse(
  request: SendMessageRequest,
  signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
  // Agent Mode override: route the entire turn into the autonomous
  // runtime instead of the single-turn chat dispatcher. The Tauri
  // stream_agent_chat command emits AgentEvents (wrapped as
  // AiStreamEvent::Agent) on the same Channel, so the consumer
  // shape is identical — only the routing differs.
  if (agentModeEnabled) {
    const firstAttachment = request.attachments?.[0];
    yield* streamAgentChat(
      {
        content: request.content,
        attachments: request.attachments,
        threadId: request.threadId,
        // No explicit recipeId — let the backend's detect_recipe
        // pick the best match from the user's message + attachments.
        // Power users / tests can extend the composer to expose an
        // override later.
        recipeId: undefined,
      },
      signal,
    );
    // Suppress unused-variable warning from the destructure above
    // until we wire per-attachment routing.
    void firstAttachment;
    return;
  }
  yield* streamAiChat(request, signal);
}

/**
 * Helper to consume the entire stream and collect all events.
 * Useful for testing or when you need all events at once.
 */
export async function collectStreamEvents(
  request: SendMessageRequest,
  signal?: AbortSignal,
): Promise<AiStreamEvent[]> {
  const events: AiStreamEvent[] = [];

  for await (const event of streamChatResponse(request, signal)) {
    events.push(event);
  }

  return events;
}
