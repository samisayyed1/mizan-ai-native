// AI Chat Streaming - Tauri-specific implementation
// Uses Tauri's Channel for efficient streaming of events from the backend

import { Channel } from "@tauri-apps/api/core";
import { tauriInvoke } from "./core";

import type {
  AiSendMessageRequest,
  AiStreamEvent,
} from "@/features/ai-assistant/types";

/**
 * Request shape for the autonomous agent runtime, mirroring the
 * Rust `AgentRunRequest`. Distinct from `AiSendMessageRequest`
 * because agent runs are not chat turns — they're invocations of
 * the recipe-driven Plan→Execute→Verify→Undo runtime.
 */
export interface AgentRunRequest {
  content: string;
  attachments?: AiSendMessageRequest["attachments"];
  threadId?: string;
  recipeId?: string;
}

/**
 * Stream AI chat responses via Tauri IPC.
 *
 * Uses Tauri's Channel for efficient streaming of events from the backend.
 *
 * @param request - The chat message request
 * @param signal - Optional AbortSignal for cancellation
 * @yields AiStreamEvent objects from the stream
 */
export async function* streamAiChat(
  request: AiSendMessageRequest,
  signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
  const channel = new Channel<AiStreamEvent>();
  const queue: AiStreamEvent[] = [];
  let done = false;
  let pendingResolve: (() => void) | null = null;

  const notifyPending = () => {
    if (pendingResolve) {
      pendingResolve();
      pendingResolve = null;
    }
  };

  channel.onmessage = (event: AiStreamEvent) => {
    queue.push(event);
    notifyPending();
  };

  const invokePromise = tauriInvoke("stream_ai_chat", {
    request,
    onEvent: channel,
  })
    .catch((err) => {
      queue.push({
        type: "error",
        threadId: "",
        runId: "",
        messageId: undefined,
        code: "network",
        message: err instanceof Error ? err.message : String(err),
      } as AiStreamEvent);
      notifyPending();
    })
    .finally(() => {
      done = true;
      notifyPending();
    });

  try {
    while (!done || queue.length > 0) {
      if (signal?.aborted) {
        break;
      }

      if (queue.length === 0) {
        await new Promise<void>((resolve) => {
          pendingResolve = resolve;
        });
        continue;
      }

      const next = queue.shift();
      if (next) {
        yield next;

        // Stop on terminal events
        if (next.type === "done" || next.type === "error") {
          return;
        }
      }
    }
  } finally {
    // @ts-expect-error - Tauri Channel doesn't have a proper cleanup method
    channel.onmessage = null;
    if (!signal?.aborted) {
      await invokePromise;
    }
  }
}

/**
 * Stream an autonomous agent run via Tauri IPC.
 *
 * Same SSE pump as `streamAiChat` but targets the
 * `stream_agent_chat` Tauri command. Events arrive on the same
 * `AiStreamEvent` union — agent-specific updates ride inside the
 * `{ type: "agent", event }` envelope variant, which the chat
 * runtime reducer matches and routes into an `agent` message part
 * (rendered by AgentProgressCard).
 *
 * The terminal events for an agent run are the SAME as for a chat
 * turn: a `done` event when the runtime emits RunComplete (the
 * Tauri command translates), or an `error` event on tier-locked
 * / no-recipe / planner-failed paths. So the consumer loop below
 * matches `streamAiChat` exactly — no special agent handling
 * needed at the transport layer.
 */
export async function* streamAgentChat(
  request: AgentRunRequest,
  signal?: AbortSignal,
): AsyncGenerator<AiStreamEvent, void, undefined> {
  const channel = new Channel<AiStreamEvent>();
  const queue: AiStreamEvent[] = [];
  let done = false;
  let pendingResolve: (() => void) | null = null;

  const notifyPending = () => {
    if (pendingResolve) {
      pendingResolve();
      pendingResolve = null;
    }
  };

  channel.onmessage = (event: AiStreamEvent) => {
    queue.push(event);
    notifyPending();
  };

  const invokePromise = tauriInvoke("stream_agent_chat", {
    request,
    onEvent: channel,
  })
    .catch((err) => {
      queue.push({
        type: "error",
        threadId: "",
        runId: "",
        messageId: undefined,
        code: "network",
        message: err instanceof Error ? err.message : String(err),
      } as AiStreamEvent);
      notifyPending();
    })
    .finally(() => {
      done = true;
      notifyPending();
    });

  try {
    while (!done || queue.length > 0) {
      if (signal?.aborted) {
        break;
      }

      if (queue.length === 0) {
        await new Promise<void>((resolve) => {
          pendingResolve = resolve;
        });
        continue;
      }

      const next = queue.shift();
      if (next) {
        yield next;

        // Stop on the run_aborted / run_complete agent terminals
        // OR the standard chat terminals (error / done) the Tauri
        // command emits on locked / no-recipe / network failures.
        if (next.type === "done" || next.type === "error") {
          return;
        }
        if (
          next.type === "agent" &&
          (next.event.type === "run_complete" ||
            next.event.type === "run_aborted")
        ) {
          return;
        }
      }
    }
  } finally {
    // @ts-expect-error - Tauri Channel doesn't have a proper cleanup method
    channel.onmessage = null;
    if (!signal?.aborted) {
      await invokePromise;
    }
  }
}
