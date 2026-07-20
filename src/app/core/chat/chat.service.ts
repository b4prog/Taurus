import { Injectable } from "@angular/core";
import { Channel, invoke } from "@tauri-apps/api/core";
import { from, map, Observable } from "rxjs";

import {
  AgentStepDto,
  ChatRequest,
  ChatResponse,
  ChatResponseDto,
  ChatStreamChunk,
  ChatStreamChunkDto,
  ChatStreamUpdate,
} from "./chat.models";

@Injectable({
  providedIn: "root",
})
export class ChatService {
  sendChatMessage(request: ChatRequest): Observable<ChatResponse> {
    return from(invoke<ChatResponseDto>("send_chat_message", { request })).pipe(
      map((response) => this.mapResponse(response)),
    );
  }

  sendChatMessageStream(request: ChatRequest): Observable<ChatStreamUpdate> {
    return new Observable<ChatStreamUpdate>((subscriber) => {
      const onChunk = new Channel<ChatStreamChunkDto>((chunk) => {
        subscriber.next({
          kind: "chunk",
          chunk: this.mapStreamChunk(chunk),
        });
      });
      const onStep = new Channel<AgentStepDto>((step) => {
        subscriber.next({ kind: "step", step });
      });

      void invoke<ChatResponseDto>("send_chat_message_stream", { request, onChunk, onStep })
        .then((response) => {
          subscriber.next({
            kind: "complete",
            response: this.mapResponse(response),
          });
          subscriber.complete();
        })
        .catch((error: unknown) => {
          subscriber.error(error);
        });

      return () => {
        onChunk.onmessage = () => {};
        onStep.onmessage = () => {};
      };
    });
  }

  private mapResponse(response: ChatResponseDto): ChatResponse {
    return {
      provider: response.provider,
      model: response.model,
      message: response.message,
      done: response.done,
      doneReason: response.done_reason,
      createdAt: response.created_at,
    };
  }

  private mapStreamChunk(chunk: ChatStreamChunkDto): ChatStreamChunk {
    return {
      provider: chunk.provider,
      model: chunk.model,
      delta: chunk.delta,
      done: chunk.done,
      doneReason: chunk.done_reason,
      createdAt: chunk.created_at,
    };
  }
}
