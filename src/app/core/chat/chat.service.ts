import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { from, map, Observable } from "rxjs";

import { ChatRequest, ChatResponse, ChatResponseDto } from "./chat.models";

@Injectable({
  providedIn: "root",
})
export class ChatService {
  sendChatMessage(request: ChatRequest): Observable<ChatResponse> {
    return from(invoke<ChatResponseDto>("send_chat_message", { request })).pipe(
      map((response) => ({
        provider: response.provider,
        model: response.model,
        message: response.message,
        done: response.done,
        doneReason: response.done_reason,
        createdAt: response.created_at,
      })),
    );
  }
}
