/**
 * AI Chat Streaming API client.
 *
 * Thin wrapper over the adapter's `streamAiChat`. The adapter handles
 * environment detection at build time (Tauri vs Web). The old agent
 * runtime + recipe planner has been removed — every turn now flows
 * through the single LLM-driven chat dispatcher, which already owns
 * the full tool surface (research_asset, create_account,
 * add_alternative_asset, delete_*, …).
 */

import { streamAiChat } from "@/adapters";
import type { AiSendMessageRequest, AiStreamEvent, AiChatModelConfig } from "../types";

export type { AiSendMessageRequest, AiStreamEvent, AiChatModelConfig };
export type SendMessageRequest = AiSendMessageRequest;
export type ChatModelConfig = AiChatModelConfig;

/**
 * Stream chat response events from the AI assistant.
 *
 * @param request - The chat message request
 * @param signal - Optional AbortSignal for cancellation
 * @yields AiStreamEvent objects from the stream
 */
export async function* streamChatResponse(
  request: SendMessageRequest,
  signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
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
