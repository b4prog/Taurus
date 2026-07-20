# Taurus

## 1. Project Overview

Taurus is a local-first, cross-platform desktop chat client built with Tauri + Angular.

The goal is to offer a ChatGPT-like interface for locally running LLM providers, starting with Ollama, while keeping the architecture provider-agnostic for future backends.

## 2. Current Status

This repository currently provides a clean foundation for Taurus MVP:

- Tauri v2 desktop shell
- Rust backend with typed provider abstraction
- Angular frontend with a chat-oriented UI shell
- Ollama health check, model listing, streaming chat, and agentic web research flow

## 3. Features

- Local-first architecture with no telemetry; external web requests happen only when the model uses a web tool
- Provider abstraction (`ChatProvider`) to support additional providers later
- Ollama integration:
  - availability check
  - automatic startup health check
  - automatic model listing when Ollama is reachable
  - model listing
  - streaming chat request/response (real-time token streaming)
- Agentic web research:
  - `search_web` searches the public web through Bing's structured RSS results
  - `fetch_web_page` extracts readable text and labeled links from a public HTTP or HTTPS page
  - the two highest-ranked search results are fetched automatically so research uses page content rather than snippets alone
  - the planner can selectively follow relevant links from hubs, indexes, directories, and overview pages to reach more specific documents
  - bounded planning rounds and tool calls fall through to a best-effort answer instead of interrupting the workflow
  - private/local network destinations and non-text downloads are rejected
- Live agent activity in the chat, with one expandable row per planning, search, page-read, source-coverage, or writing step
- Typed command contracts between Angular and Rust
- Basic chat workspace UI:
  - conversation sidebar placeholder
  - model selection
  - provider status panel
  - prompt input and send button

## 4. Architecture Overview

Frontend (Angular):

- UI components call typed services
- Services call Tauri commands via `@tauri-apps/api`
- Command failures are surfaced to users with friendly errors

Backend (Rust):

- Tauri commands are thin and validate input
- Provider-independent types live in `providers/mod.rs`
- Ollama integration stays in `providers/ollama.rs`
- `agent.rs` coordinates model/tool turns without coupling the UI to Ollama
- Web tool execution and network safety rules live under `tools/`
- Shared application state stores provider and tool instances

## 5. Repository Structure

```text
.
├── AGENTS.md
├── README.md
├── angular.json
├── package.json
├── src/
│   └── app/
│       ├── core/
│       │   ├── chat/
│       │   ├── providers/
│       │   └── tauri/
│       ├── features/
│       │   ├── chat/
│       │   └── settings/
│       ├── app.component.*
│       ├── app.config.ts
│       └── app.routes.ts
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    └── src/
        ├── app_state.rs
        ├── agent.rs
        ├── error.rs
        ├── commands/
        │   ├── chat.rs
        │   ├── health.rs
        │   ├── mod.rs
        │   └── models.rs
        ├── providers/
            ├── mod.rs
            └── ollama.rs
        └── tools/
            ├── mod.rs
            └── web.rs
```

## 6. Requirements

You need:

- Git
- Node.js + npm
- Rust toolchain (`rustup`, `cargo`)
- Platform dependencies required by Tauri
- Ollama installed locally

## 7. Installing Dependencies

Install platform prerequisites first, then project dependencies.

High-level order:

1. Install Rust.
2. Install Node.js.
3. Install Tauri system dependencies.
4. Install Ollama.
5. Install npm dependencies in this repo.

## 8. Installing Rust

Recommended:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation:

```bash
rustup update
rustup default stable
cargo --version
```

## 9. Installing Node.js

Install an LTS version (Node 20.x recommended for this repo).

Examples:

- macOS/Linux with nvm:

```bash
nvm install --lts
nvm use --lts
node -v
npm -v
```

- Windows:
  - Install Node.js LTS from [nodejs.org](https://nodejs.org/)
  - Reopen terminal and check:

```bash
node -v
npm -v
```

## 10. Installing System Dependencies for Tauri

Official prerequisites reference:

- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)

### macOS

- Install Xcode Command Line Tools:

```bash
xcode-select --install
```

### Windows

- Install Microsoft Visual Studio C++ Build Tools (Desktop development with C++)
- Install WebView2 runtime (usually preinstalled on Windows 11)
- Install Rust MSVC target via rustup if needed

### Linux

Package names vary by distro.

For Debian/Ubuntu, a common setup is:

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

For Fedora:

```bash
sudo dnf install -y \
  webkit2gtk4.1-devel \
  gtk3-devel \
  libayatana-appindicator-gtk3-devel \
  librsvg2-devel
```

For Arch Linux:

```bash
sudo pacman -S --needed \
  webkit2gtk-4.1 \
  gtk3 \
  libayatana-appindicator \
  librsvg
```

## 11. Installing Ollama

Install Ollama from:

- [https://ollama.com/download](https://ollama.com/download)

Then start Ollama and confirm it is running:

```bash
ollama --version
```

By default, Taurus uses:

```text
http://localhost:11434
```

Override with:

```bash
export TAURUS_OLLAMA_BASE_URL=http://localhost:11434
```

On Windows PowerShell:

```powershell
$env:TAURUS_OLLAMA_BASE_URL = "http://localhost:11434"
```

## 12. Pulling an Ollama Model

Example:

```bash
ollama pull llama3.1:8b
ollama list
```

## 13. Installing Project Dependencies

From the repository root:

```bash
npm install
```

This will generate and update `package-lock.json`.

## 14. Using Web Research

Choose an Ollama model that supports tool calling. When a prompt needs current or external information, the model can search the web. Taurus then fetches the two highest-ranked result pages so the model can use page content in its final response. If a fetched result is a hub, index, directory, feed, listing, or overview, Taurus exposes its labeled links to the planner so the planner can open only the documents relevant to the request. For synthesis, comparison, explanation, and evaluation tasks, headlines and short summaries on a hub are treated as discovery material; the planner is instructed to fetch a small representative set of the linked documents before answering. This behavior is generic and is not tied to news or any particular website structure.

Each action appears above the assistant response as a one-line status row. Select a row to expand or collapse it. Search details include the exact query, result titles, URLs, and snippets. Page-read details include the fetched text and a bounded list of discovered links. The source-coverage step reports how many search result sets and pages were collected, whether more evidence is needed, and the exact next action.

Web research is opt-in at the prompt level: the model decides when it is needed. Search queries are sent to Bing, and the highest-ranked result pages are requested directly from their hosts. Do not include secrets in a prompt that asks for web research.

## 15. Running Taurus in Development Mode

Run the desktop app (Angular + Tauri):

```bash
npm run tauri:dev
```

Useful alternatives:

```bash
npm run start
npm run build
```

## 16. Building the Application

Production desktop build:

```bash
npm run tauri:build
```

Frontend-only production build:

```bash
npm run build
```

## 17. Running Tests

Frontend unit tests:

```bash
npm run test
```

Rust tests:

```bash
npm run rust:test
```

If frontend tests fail due missing browser runtime, install Chrome/Chromium and retry.

## 18. Formatting the Code

Format and check formatting:

```bash
npm run format
npm run format:check
npm run rust:fmt
```

## 19. Linting the Code

Frontend lint:

```bash
npm run lint
```

Rust lint:

```bash
npm run rust:clippy
```

## 20. Type Checking

Run TypeScript type check:

```bash
npm run typecheck
```

## 21. Troubleshooting

Common issues:

- Ollama unavailable:
  - Make sure Ollama is running.
  - Verify `TAURUS_OLLAMA_BASE_URL`.
  - Confirm `http://localhost:11434/api/tags` is reachable.
- Agent planning fails with a tool-calling error:
  - Select an Ollama model with tool-calling support.
  - Confirm the model can use tools through Ollama's `/api/chat` endpoint.
- Web search or page fetch fails:
  - Confirm the machine has internet access.
  - Bing or the target site may reject automated requests or apply rate limits.
  - Localhost, private IP ranges, oversized responses, and binary content are blocked intentionally.
- Linux build errors about WebKit/GTK:
  - Recheck distro package names from Tauri prerequisites docs.
- `npm run test` fails in headless environments:
  - Install Chrome/Chromium or run tests in CI with a headless browser image.
- Tauri build fails on Windows:
  - Confirm Visual Studio C++ build tools are installed.
- Rust toolchain errors:
  - Run `rustup update`.
- `ng build` crashes with `malloc: ... pointer being freed was not allocated`:
  - This has been observed with some Node 22 environments.
  - Use Node 20 LTS for build/test commands.
  - One-off workaround:

```bash
npx -y node@20 ./node_modules/@angular/cli/bin/ng build
```

## 22. Security Notes

- Taurus is local-first and does not include telemetry by default.
- Web research sends the model's search query to Bing and automatically fetches up to two highest-ranked public result pages per search. The planner may selectively request additional public pages linked by fetched results, subject to the global planning-round and tool-call limits.
- The fetch tool blocks loopback, private, link-local, and other special-use IP ranges, including across redirects.
- Web responses are size-limited and page text is treated as untrusted reference material in the agent prompt. The expandable workflow retains the fetched text, while the model receives a bounded excerpt to leave enough context for its answer.
- Tauri command surface is intentionally narrow:
  - no arbitrary shell execution
  - no arbitrary filesystem commands
- Frontend input is treated as untrusted and validated on the Rust side.
- Provider-specific networking is isolated to provider modules.

## 22. Roadmap

Planned next steps include:

- Conversation persistence
- Provider selection in UI
- Additional provider modules (OpenAI-compatible/local backends)
- Explicit-permission tool calling
- Local settings management

## 23. License

This project is licensed under the MIT License. See `LICENSE`.
