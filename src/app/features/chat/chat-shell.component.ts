import { CommonModule } from "@angular/common";
import { Component, inject, NgZone, OnInit } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { finalize } from "rxjs";

import { ChatMessage } from "../../core/chat/chat.models";
import { ChatService } from "../../core/chat/chat.service";
import { ModelInfo, ProviderHealth } from "../../core/providers/provider.models";
import { ProviderService } from "../../core/providers/provider.service";
import { extractTauriErrorMessage } from "../../core/tauri/tauri-api-error";

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

  protected readonly providerName = "ollama";

  protected health: ProviderHealth | null = null;
  protected healthError = "";
  protected isCheckingHealth = false;

  protected models: ModelInfo[] = [];
  protected modelsError = "";
  protected isLoadingModels = false;

  protected messages: ChatMessage[] = [];

  protected prompt = "";
  protected selectedModel = "";
  protected temperature = 0.7;
  protected isSending = false;
  protected chatError = "";
  protected lastDoneReason = "";

  ngOnInit(): void {
    this.checkOllamaHealth(true);
  }

  protected checkOllamaHealth(loadModelsOnSuccess = false): void {
    this.healthError = "";
    this.isCheckingHealth = true;

    this.providerService
      .checkOllamaHealth()
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
      .listOllamaModels()
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
    if (trimmedPrompt.length === 0) {
      this.chatError = "Enter a message before sending.";
      return;
    }

    if (this.selectedModel.length === 0) {
      this.chatError = "Select a model before sending a message.";
      return;
    }

    this.chatError = "";
    this.lastDoneReason = "";

    const userMessage: ChatMessage = {
      role: "user",
      content: trimmedPrompt,
    };

    const nextMessages = [...this.messages, userMessage];
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
        provider: this.providerName,
        model: this.selectedModel,
        messages: nextMessages,
        temperature: this.temperature,
        stream: true,
      })
      .pipe(finalize(() => (this.isSending = false)))
      .subscribe({
        next: (update) => {
          this.ngZone.run(() => {
            if (update.kind === "chunk") {
              const currentAssistant = this.messages.at(assistantIndex);
              if (currentAssistant === undefined) {
                return;
              }

              if (update.chunk.delta.length > 0) {
                currentAssistant.content = `${currentAssistant.content}${update.chunk.delta}`;
                this.messages = [...this.messages];
              }

              if (update.chunk.done) {
                this.lastDoneReason = update.chunk.doneReason ?? "completed";
              }
              return;
            }

            const currentAssistant = this.messages.at(assistantIndex);
            const fallbackContent = currentAssistant?.content ?? "";
            const finalContent =
              update.response.message.content.length > 0
                ? update.response.message.content
                : fallbackContent;

            this.messages[assistantIndex] = {
              ...update.response.message,
              content: finalContent,
            };
            this.messages = [...this.messages];
            this.lastDoneReason = update.response.doneReason ?? "completed";
          });
        },
        error: (error: unknown) => {
          this.ngZone.run(() => {
            this.chatError = extractTauriErrorMessage(error);
            this.messages = nextMessages;
          });
        },
      });
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
}
