import { ComponentFixture, TestBed } from "@angular/core/testing";
import { of } from "rxjs";
import { ChatResponse, ChatStreamUpdate } from "../../core/chat/chat.models";
import { ChatService } from "../../core/chat/chat.service";
import { ProviderService } from "../../core/providers/provider.service";
import { ChatShellComponent } from "./chat-shell.component";

describe("ChatShellComponent", () => {
  let fixture: ComponentFixture<ChatShellComponent>;
  let component: ChatShellComponent;
  const providerServiceSpy = jasmine.createSpyObj<ProviderService>("ProviderService", [
    "checkProviderHealth",
    "listProviderModels",
  ]);
  const chatServiceSpy = jasmine.createSpyObj<ChatService>("ChatService", [
    "sendChatMessageStream",
  ]);

  beforeEach(async () => {
    providerServiceSpy.checkProviderHealth.calls.reset();
    providerServiceSpy.listProviderModels.calls.reset();
    chatServiceSpy.sendChatMessageStream.calls.reset();
    providerServiceSpy.checkProviderHealth.and.returnValue(
      of({
        provider: "ollama",
        healthy: true,
        baseUrl: "http://localhost:11434",
        message: "ok",
      }),
    );
    providerServiceSpy.listProviderModels.and.returnValue(
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
    const streamUpdates: ChatStreamUpdate[] = [
      {
        kind: "step",
        step: {
          id: "step-1",
          label: "Searching the web",
          status: "running",
          detail: "Query: Taurus",
        },
      },
      {
        kind: "step",
        step: {
          id: "step-1",
          label: "Searching the web",
          status: "completed",
          detail: "Found 5 web search results.",
        },
      },
      {
        kind: "chunk",
        chunk: {
          provider: "ollama",
          model: "llama3:latest",
          delta: "Hello from",
          done: false,
          doneReason: null,
          createdAt: null,
        },
      },
      {
        kind: "chunk",
        chunk: {
          provider: "ollama",
          model: "llama3:latest",
          delta: " model",
          done: true,
          doneReason: "stop",
          createdAt: null,
        },
      },
      {
        kind: "complete",
        response: {
          provider: "ollama",
          model: "llama3:latest",
          message: { role: "assistant", content: "Hello from model" },
          done: true,
          doneReason: "stop",
          createdAt: null,
        } as ChatResponse,
      },
    ];
    chatServiceSpy.sendChatMessageStream.and.returnValue(of(...streamUpdates));
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

  it("checks Ollama and loads models on startup", () => {
    const vm = component as unknown as {
      selectedModel: string;
      models: Array<{ id: string }>;
      messages: Array<{ role: string; content: string }>;
    };
    expect(providerServiceSpy.checkProviderHealth).toHaveBeenCalledWith("ollama");
    expect(providerServiceSpy.listProviderModels).toHaveBeenCalledWith("ollama");
    expect(providerServiceSpy.checkProviderHealth).toHaveBeenCalledTimes(1);
    expect(providerServiceSpy.listProviderModels).toHaveBeenCalledTimes(1);
    expect(vm.models.length).toBe(1);
    expect(vm.selectedModel).toBe("llama3:latest");
    expect(vm.messages.length).toBe(0);
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
    expect(chatServiceSpy.sendChatMessageStream).not.toHaveBeenCalled();
  });

  it("sanitizes temperature before sending", () => {
    const vm = component as unknown as {
      prompt: string;
      selectedModel: string;
      temperature: number | null;
      sendPrompt: () => void;
    };

    vm.selectedModel = "llama3:latest";

    vm.prompt = "Hello";
    vm.temperature = null;
    vm.sendPrompt();

    expect(chatServiceSpy.sendChatMessageStream).toHaveBeenCalled();
    const firstRequest = chatServiceSpy.sendChatMessageStream.calls.mostRecent().args[0] as {
      temperature: number;
    };
    expect(firstRequest.temperature).toBe(0.7);

    chatServiceSpy.sendChatMessageStream.calls.reset();

    vm.prompt = "Hello again";
    vm.temperature = 99;
    vm.sendPrompt();

    expect(chatServiceSpy.sendChatMessageStream).toHaveBeenCalled();
    const secondRequest = chatServiceSpy.sendChatMessageStream.calls.mostRecent().args[0] as {
      temperature: number;
    };
    expect(secondRequest.temperature).toBe(2);
  });

  it("streams assistant response while sending", () => {
    const vm = component as unknown as {
      prompt: string;
      selectedModel: string;
      messages: Array<{ role: string; content: string }>;
      sendPrompt: () => void;
      lastDoneReason: string;
    };
    const initialLength = vm.messages.length;
    vm.selectedModel = "llama3:latest";
    vm.prompt = "Hello";
    vm.sendPrompt();
    expect(chatServiceSpy.sendChatMessageStream).toHaveBeenCalled();
    expect(vm.messages.length).toBe(initialLength + 2);
    expect(vm.messages.at(-1)?.role).toBe("assistant");
    expect(vm.messages.at(-1)?.content).toBe("Hello from model");
    expect(vm.lastDoneReason).toBe("stop");
  });

  it("updates each agent step in place and toggles its detail", () => {
    const vm = component as unknown as {
      prompt: string;
      selectedModel: string;
      messages: Array<{ role: string; content: string }>;
      sendPrompt: () => void;
      workflowSteps: (messageIndex: number) => Array<{
        id: string;
        status: string;
        expanded: boolean;
      }>;
      toggleStepDetail: (messageIndex: number, stepId: string) => void;
    };
    vm.selectedModel = "llama3:latest";
    vm.prompt = "Find Taurus";
    vm.sendPrompt();
    const assistantIndex = vm.messages.length - 1;
    expect(vm.workflowSteps(assistantIndex).length).toBe(1);
    expect(vm.workflowSteps(assistantIndex)[0].status).toBe("completed");
    expect(vm.workflowSteps(assistantIndex)[0].expanded).toBeFalse();
    vm.toggleStepDetail(assistantIndex, "step-1");
    expect(vm.workflowSteps(assistantIndex)[0].expanded).toBeTrue();
  });
});
