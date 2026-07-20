export type ChatRole = "system" | "user" | "assistant" | "tool";

export interface ChatMessage {
  role: ChatRole;
  content: string;
}

export type AgentStepStatus = "running" | "completed" | "failed";

export interface AgentStepDto {
  id: string;
  label: string;
  status: AgentStepStatus;
  detail: string;
}

export interface AgentStep extends AgentStepDto {
  expanded: boolean;
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

export interface ChatStreamChunkDto {
  provider: string;
  model: string;
  delta: string;
  done: boolean;
  done_reason: string | null;
  created_at: string | null;
}

export interface ChatStreamChunk {
  provider: string;
  model: string;
  delta: string;
  done: boolean;
  doneReason: string | null;
  createdAt: string | null;
}

export type ChatStreamUpdate =
  | { kind: "chunk"; chunk: ChatStreamChunk }
  | { kind: "step"; step: AgentStepDto }
  | { kind: "complete"; response: ChatResponse };
