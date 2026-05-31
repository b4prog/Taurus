import { ComponentFixture, TestBed } from "@angular/core/testing";
import { of } from "rxjs";

import { ChatResponse } from "../../core/chat/chat.models";
import { ChatService } from "../../core/chat/chat.service";
import { ProviderService } from "../../core/providers/provider.service";
import { ChatShellComponent } from "./chat-shell.component";

describe("ChatShellComponent", () => {
  let fixture: ComponentFixture<ChatShellComponent>;
  let component: ChatShellComponent;

  const providerServiceSpy = jasmine.createSpyObj<ProviderService>("ProviderService", [
    "checkOllamaHealth",
    "listOllamaModels",
  ]);

  const chatServiceSpy = jasmine.createSpyObj<ChatService>("ChatService", ["sendChatMessage"]);

  beforeEach(async () => {
    providerServiceSpy.checkOllamaHealth.calls.reset();
    providerServiceSpy.listOllamaModels.calls.reset();
    chatServiceSpy.sendChatMessage.calls.reset();

    providerServiceSpy.checkOllamaHealth.and.returnValue(
      of({
        provider: "ollama",
        healthy: true,
        baseUrl: "http://localhost:11434",
        message: "ok",
      }),
    );

    providerServiceSpy.listOllamaModels.and.returnValue(
      of([
        {
          provider: "ollama",
          id: "llama3:latest",
          displayName: "llama3:latest",
          sizeBytes: 10,
          modifiedAt: null,
        },
      ]),
    );

    chatServiceSpy.sendChatMessage.and.returnValue(
      of({
        provider: "ollama",
        model: "llama3:latest",
        message: { role: "assistant", content: "Hello from model" },
        done: true,
        doneReason: "stop",
        createdAt: null,
      } as ChatResponse),
    );

    await TestBed.configureTestingModule({
      imports: [ChatShellComponent],
      providers: [
        { provide: ProviderService, useValue: providerServiceSpy },
        { provide: ChatService, useValue: chatServiceSpy },
      ],
    }).compileComponents();

    fixture = TestBed.createComponent(ChatShellComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it("creates the component", () => {
    expect(component).toBeTruthy();
  });

  it("requires model selection before sending", () => {
    const vm = component as unknown as {
      prompt: string;
      selectedModel: string;
      chatError: string;
      sendPrompt: () => void;
    };

    vm.prompt = "Tell me about Taurus";
    vm.selectedModel = "";
    vm.sendPrompt();

    expect(vm.chatError).toContain("Select a model");
    expect(chatServiceSpy.sendChatMessage).not.toHaveBeenCalled();
  });

  it("appends assistant response after successful send", () => {
    const vm = component as unknown as {
      prompt: string;
      selectedModel: string;
      messages: Array<{ role: string; content: string }>;
      sendPrompt: () => void;
    };

    const initialLength = vm.messages.length;
    vm.selectedModel = "llama3:latest";
    vm.prompt = "Hello";
    vm.sendPrompt();

    expect(chatServiceSpy.sendChatMessage).toHaveBeenCalled();
    expect(vm.messages.length).toBe(initialLength + 2);
    expect(vm.messages.at(-1)?.role).toBe("assistant");
  });
});
