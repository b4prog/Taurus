import { CommonModule } from "@angular/common";
import { Component, inject, NgZone, OnInit } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { finalize } from "rxjs";

import {
  AgentStep,
  AgentStepDto,
  ChatMessage,
  ChatStreamChunk,
  ChatStreamUpdate,
} from "../../core/chat/chat.models";
import { ChatService } from "../../core/chat/chat.service";
import { ModelInfo, ProviderHealth } from "../../core/providers/provider.models";
import { ProviderService } from "../../core/providers/provider.service";
import { extractTauriErrorMessage } from "../../core/tauri/tauri-api-error";

const DEFAULT_PROVIDER_KEY = "ollama";
const DEFAULT_TEMPERATURE = 0.7;
const MIN_TEMPERATURE = 0.0;
const MAX_TEMPERATURE = 2.0;

@Component({
  selector: "app-chat-shell",
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: "./chat-shell.component.html",
  styleUrl: "./chat-shell.component.css",
})
export class ChatShellComponent implements OnInit {
  private readonly providerService = inject(ProviderService);
  private readonly chatService = inject(ChatService);
  private readonly ngZone = inject(NgZone);

  protected providerKey = DEFAULT_PROVIDER_KEY;

  protected health: ProviderHealth | null = null;
  protected healthError = "";
  protected isCheckingHealth = false;

  protected models: ModelInfo[] = [];
  protected modelsError = "";
  protected isLoadingModels = false;

  protected messages: ChatMessage[] = [];
  protected workflows = new Map<number, AgentStep[]>();

  protected prompt = "";
  protected selectedModel = "";
  protected temperature: number | null = DEFAULT_TEMPERATURE;
  protected isSending = false;
  protected chatError = "";
  protected lastDoneReason = "";

  ngOnInit(): void {
    this.checkProviderHealth(true);
  }

  protected checkProviderHealth(loadModelsOnSuccess = false): void {
    this.healthError = "";
    this.isCheckingHealth = true;

    this.providerService
      .checkProviderHealth(this.providerKey)
      .pipe(finalize(() => (this.isCheckingHealth = false)))
      .subscribe({
        next: (health) => {
          this.health = health;
          if (loadModelsOnSuccess && health.healthy) {
            this.loadModels();
          }
        },
        error: (error: unknown) => {
          this.health = null;
          this.healthError = extractTauriErrorMessage(error);
        },
      });
  }

  protected loadModels(): void {
    this.modelsError = "";
    this.isLoadingModels = true;

    this.providerService
      .listProviderModels(this.providerKey)
      .pipe(finalize(() => (this.isLoadingModels = false)))
      .subscribe({
        next: (models) => {
          this.models = models;

          if (
            this.selectedModel === "" ||
            !models.some((model) => model.id === this.selectedModel)
          ) {
            this.selectedModel = models.at(0)?.id ?? "";
          }
        },
        error: (error: unknown) => {
          this.models = [];
          this.modelsError = extractTauriErrorMessage(error);
        },
      });
  }

  protected sendPrompt(): void {
    const trimmedPrompt = this.prompt.trim();
    if (!this.canSendPrompt(trimmedPrompt)) {
      return;
    }
    this.chatError = "";
    this.lastDoneReason = "";
    const userMessage: ChatMessage = {
      role: "user",
      content: trimmedPrompt,
    };
    const nextMessages = [...this.messages, userMessage];
    const requestMessages = [
      ...this.messages.filter((message) => message.content.trim().length > 0),
      userMessage,
    ];
    const streamingAssistantMessage: ChatMessage = {
      role: "assistant",
      content: "",
    };
    const streamMessages = [...nextMessages, streamingAssistantMessage];
    const assistantIndex = streamMessages.length - 1;
    this.messages = streamMessages;
    this.prompt = "";
    this.isSending = true;
    this.chatService
      .sendChatMessageStream({
        provider: this.providerKey,
        model: this.selectedModel,
        messages: requestMessages,
        temperature: this.safeTemperature(),
        stream: true,
      })
      .pipe(finalize(() => (this.isSending = false)))
      .subscribe({
        next: (update) => {
          this.ngZone.run(() => this.applyStreamUpdate(assistantIndex, update));
        },
        error: (error: unknown) => {
          this.ngZone.run(() => {
            this.chatError = extractTauriErrorMessage(error);
          });
        },
      });
  }

  protected workflowSteps(messageIndex: number): AgentStep[] {
    return this.workflows.get(messageIndex) ?? [];
  }

  protected toggleStepDetail(messageIndex: number, stepId: string): void {
    const steps = this.workflowSteps(messageIndex).map((step) =>
      step.id === stepId ? { ...step, expanded: !step.expanded } : step,
    );
    this.workflows = new Map(this.workflows).set(messageIndex, steps);
  }

  protected stepStatusLabel(step: AgentStep): string {
    if (step.status === "running") {
      return "In progress";
    }
    if (step.status === "failed") {
      return "Failed";
    }
    return "Completed";
  }

  protected trackByModelId(_: number, model: ModelInfo): string {
    return model.id;
  }

  protected trackByMessageIndex(index: number): number {
    return index;
  }

  protected authorLabel(message: ChatMessage): string {
    if (message.role === "assistant") {
      return "Taurus";
    }

    if (message.role === "user") {
      return "You";
    }

    return "System";
  }

  protected formatModelSize(sizeBytes: number | null): string {
    if (sizeBytes === null) {
      return "size unavailable";
    }

    const sizeInGiB = sizeBytes / 1024 ** 3;
    return `${sizeInGiB.toFixed(2)} GiB`;
  }

  private canSendPrompt(trimmedPrompt: string): boolean {
    if (trimmedPrompt.length === 0) {
      this.chatError = "Enter a message before sending.";
      return false;
    }
    if (this.selectedModel.length === 0) {
      this.chatError = "Select a model before sending a message.";
      return false;
    }
    return true;
  }

  private safeTemperature(): number {
    const rawTemperature =
      this.temperature === null || this.temperature === undefined
        ? DEFAULT_TEMPERATURE
        : Number(this.temperature);
    return Number.isFinite(rawTemperature)
      ? Math.max(MIN_TEMPERATURE, Math.min(MAX_TEMPERATURE, rawTemperature))
      : DEFAULT_TEMPERATURE;
  }

  private applyStreamUpdate(assistantIndex: number, update: ChatStreamUpdate): void {
    if (update.kind === "step") {
      this.applyAgentStep(assistantIndex, update.step);
      return;
    }
    if (update.kind === "chunk") {
      this.applyStreamChunk(assistantIndex, update.chunk);
      return;
    }
    const currentAssistant = this.messages.at(assistantIndex);
    const fallbackContent = currentAssistant?.content ?? "";
    this.messages[assistantIndex] = {
      ...update.response.message,
      content:
        update.response.message.content.length > 0
          ? update.response.message.content
          : fallbackContent,
    };
    this.messages = [...this.messages];
    this.lastDoneReason = update.response.doneReason ?? "completed";
  }

  private applyAgentStep(assistantIndex: number, event: AgentStepDto): void {
    const currentSteps = this.workflowSteps(assistantIndex);
    const existingIndex = currentSteps.findIndex((step) => step.id === event.id);
    const updatedStep: AgentStep = {
      ...event,
      expanded: existingIndex >= 0 ? currentSteps[existingIndex].expanded : false,
    };
    const updatedSteps = [...currentSteps];
    if (existingIndex >= 0) {
      updatedSteps[existingIndex] = updatedStep;
    } else {
      updatedSteps.push(updatedStep);
    }
    this.workflows = new Map(this.workflows).set(assistantIndex, updatedSteps);
  }

  private applyStreamChunk(assistantIndex: number, chunk: ChatStreamChunk): void {
    const currentAssistant = this.messages.at(assistantIndex);
    if (currentAssistant === undefined) {
      return;
    }
    if (chunk.delta.length > 0) {
      currentAssistant.content = `${currentAssistant.content}${chunk.delta}`;
      this.messages = [...this.messages];
    }
    if (chunk.done) {
      this.lastDoneReason = chunk.doneReason ?? "completed";
    }
  }
}
