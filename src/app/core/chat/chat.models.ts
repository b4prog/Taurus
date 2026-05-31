export type ChatRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatRole;
  content: string;
}

export interface ChatRequest {
  provider?: string;
  model: string;
  messages: ChatMessage[];
  temperature?: number;
  stream?: boolean;
}

export interface ChatResponseDto {
  provider: string;
  model: string;
  message: ChatMessage;
  done: boolean;
  done_reason: string | null;
  created_at: string | null;
}

export interface ChatResponse {
  provider: string;
  model: string;
  message: ChatMessage;
  done: boolean;
  doneReason: string | null;
  createdAt: string | null;
}
