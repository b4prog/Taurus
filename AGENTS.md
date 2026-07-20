# AGENTS.md for Taurus

This file contains durable instructions for AI coding agents working on **Taurus**.

Taurus is a cross-platform desktop application that provides a local-first ChatGPT-like interface for interacting with LLMs. The first supported provider is Ollama, but the codebase must remain provider-agnostic so additional local or remote providers can be added later.

## Stack

- Desktop shell: Tauri v2
- Backend: Rust
- Frontend: Angular with TypeScript
- Initial LLM provider: Ollama via its local HTTP API
- Target platforms: macOS, Windows, Linux

## Core principles

1. Keep the application local-first.
2. Do not couple the whole application to Ollama.
3. Keep Rust backend code strongly typed and organized by responsibility.
4. Keep Angular UI code strict, modular, and maintainable.
5. Treat all data from the frontend as untrusted.
6. Keep Tauri permissions minimal.
7. Do not expose generic shell execution or arbitrary filesystem access.
8. Do not add telemetry or external services unless explicitly requested and documented.

## Expected repository organization

Use a conventional Tauri + Angular structure.

Recommended backend organization:

```text
src-tauri/
  src/
    commands/
      mod.rs
      health.rs
      models.rs
      chat.rs
    providers/
      mod.rs
      ollama.rs
    app_state.rs
    error.rs
    lib.rs
    main.rs
```

Recommended frontend organization:

```text
src/
  app/
    core/
      tauri/
      chat/
      providers/
    features/
      chat/
      settings/
    shared/
      ui/
    app.config.ts
    app.routes.ts
```

The exact layout may evolve, but keep responsibilities clear.

## Rust backend guidelines

- Prefer explicit domain types over loosely typed maps or raw JSON values.
- Use `serde` for data exchanged with the frontend and providers.
- Use a project-specific error type such as `AppError`.
- Return user-friendly serialized errors to the frontend.
- Avoid `unwrap()` and `expect()` in normal application code.
- Use async code carefully and avoid blocking the Tauri runtime.
- Keep provider-specific code inside `providers/`.
- Keep Tauri command handlers thin; they should validate input and delegate to application services/providers.

## LLM provider design

Do not place provider-specific assumptions in the UI or generic chat logic.

A provider should expose capabilities similar to:

```rust
#[async_trait::async_trait]
pub trait ChatProvider: Send + Sync {
    async fn health_check(&self) -> Result<ProviderHealth, AppError>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, AppError>;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError>;
}
```

When adding a new provider:

1. Add a new module under `src-tauri/src/providers/`.
2. Implement the provider trait.
3. Add provider-specific configuration without breaking existing defaults.
4. Add tests for request/response mapping and error handling.
5. Update the README.
6. Update UI provider/model selection only through generic provider types.

## Ollama guidelines

The default Ollama base URL is:

```text
http://localhost:11434
```

Support overriding it with configuration or an environment variable such as:

```text
TAURUS_OLLAMA_BASE_URL=http://localhost:11434
```

If Ollama is not reachable, return a clear error that the UI can display. The application must not crash.

## Angular frontend guidelines

- Use strict TypeScript.
- Prefer standalone components.
- Use services for Tauri command calls.
- Keep UI components focused and small.
- Avoid `any` unless there is a documented reason.
- Keep request/response models typed.
- Keep chat state management simple until complexity justifies a dedicated store.
- Use readable templates and meaningful names.

## Tauri command guidelines

Tauri commands are a security boundary.

Rules:

1. Expose only specific commands.
2. Validate command inputs.
3. Do not expose generic shell execution.
4. Do not expose arbitrary filesystem access.
5. Do not pass raw frontend data into OS APIs.
6. Return typed errors.
7. Keep commands thin and delegate to backend services.

Good command examples:

- `check_ollama_health`
- `list_ollama_models`
- `send_chat_message`

Bad command examples:

- `run_command(command: String)`
- `read_any_file(path: String)`
- `write_any_file(path: String, contents: String)`

## Security expectations

- Keep Tauri permissions minimal.
- Treat prompt text and conversation content as potentially sensitive.
- Do not log full prompts or responses by default.
- Do not add analytics or telemetry by default.
- If future tool-calling features are added, require explicit user confirmation before performing filesystem, process, network, or other sensitive actions.
- Document any new permission or external dependency in the README.

## Documentation expectations

The README must stay beginner-friendly and complete.

Whenever changing setup, build, dependencies, or platform requirements, update the README.

The README should explain:

- How to install prerequisites
- How to install project dependencies
- How to run the app in development mode
- How to build the app
- How to format code
- How to lint code
- How to run tests
- How to troubleshoot common issues

## Quality checks

Before considering work complete, run the applicable commands and fix issues:

```bash
npm install
npm run format:check
npm run lint
npm run build
npm run rust:fmt
npm run rust:clippy
npm run rust:test
npm run tauri:build
```

If a command cannot be run in the current environment, clearly report it and explain why.

## Formatting and linting

- Rust code must pass `cargo fmt`.
- Rust code should pass `cargo clippy --all-targets --all-features -- -D warnings`.
- TypeScript/Angular code must pass configured linting.
- Frontend formatting should use Prettier.
- Do not introduce formatting-only noise in unrelated files.

## Testing expectations

Add or update tests when changing behavior.

Minimum expectations:

- Rust tests for provider configuration, response parsing, and error mapping where practical.
- Angular tests for services/components where practical.
- Manual testing notes for flows that are difficult to automate.

## Git hygiene

Do not commit build outputs, caches, logs, or local editor files.

Do commit important lockfiles and configuration files, including:

- `Cargo.lock`
- `package-lock.json` or the selected package manager lockfile
- Tauri configuration
- Angular configuration
- lint/format/test configuration

## Product direction

The initial MVP should focus on:

1. Detecting Ollama.
2. Listing local Ollama models.
3. Providing a basic chat UI shell.
4. Sending a prompt to a selected model.
5. Displaying responses clearly.
6. Establishing a clean provider abstraction.

Future features may include:

- Streaming token display
- Conversation persistence
- Prompt templates
- Provider selection
- OpenAI-compatible providers
- Local document ingestion
- Embeddings and RAG
- Tool-calling with explicit permissions
- Secure local settings storage

Keep the foundation simple, safe, and extensible.
