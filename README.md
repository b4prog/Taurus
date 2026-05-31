# Taurus

## 1. Project Overview

Taurus is a local-first, cross-platform desktop chat client built with Tauri + Angular.

The goal is to offer a ChatGPT-like interface for locally running LLM providers, starting with Ollama, while keeping the architecture provider-agnostic for future backends.

## 2. Current Status

This repository currently provides a clean foundation for Taurus MVP:

- Tauri v2 desktop shell
- Rust backend with typed provider abstraction
- Angular frontend with a chat-oriented UI shell
- Ollama health check, model listing, and non-streaming chat command flow

## 3. Features

- Local-first architecture (no telemetry and no remote SaaS dependencies by default)
- Provider abstraction (`ChatProvider`) to support additional providers later
- Ollama integration:
  - availability check
  - model listing
  - chat request/response
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
- Shared application state stores provider instances

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
        ├── error.rs
        ├── commands/
        │   ├── chat.rs
        │   ├── health.rs
        │   ├── mod.rs
        │   └── models.rs
        └── providers/
            ├── mod.rs
            └── ollama.rs
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

## 14. Running Taurus in Development Mode

Run the desktop app (Angular + Tauri):

```bash
npm run tauri:dev
```

Useful alternatives:

```bash
npm run start
npm run build
```

## 15. Building the Application

Production desktop build:

```bash
npm run tauri:build
```

Frontend-only production build:

```bash
npm run build
```

## 16. Running Tests

Frontend unit tests:

```bash
npm run test
```

Rust tests:

```bash
npm run rust:test
```

If frontend tests fail due missing browser runtime, install Chrome/Chromium and retry.

## 17. Formatting the Code

Format and check formatting:

```bash
npm run format
npm run format:check
npm run rust:fmt
```

## 18. Linting the Code

Frontend lint:

```bash
npm run lint
```

Rust lint:

```bash
npm run rust:clippy
```

## 19. Type Checking

Run TypeScript type check:

```bash
npm run typecheck
```

## 20. Troubleshooting

Common issues:

- Ollama unavailable:
  - Make sure Ollama is running.
  - Verify `TAURUS_OLLAMA_BASE_URL`.
  - Confirm `http://localhost:11434/api/tags` is reachable.
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

## 21. Security Notes

- Taurus is local-first and does not include telemetry by default.
- Tauri command surface is intentionally narrow:
  - no arbitrary shell execution
  - no arbitrary filesystem commands
- Frontend input is treated as untrusted and validated on the Rust side.
- Provider-specific networking is isolated to provider modules.

## 22. Roadmap

Planned next steps include:

- Streaming response tokens in the chat UI
- Conversation persistence
- Provider selection in UI
- Additional provider modules (OpenAI-compatible/local backends)
- Explicit-permission tool calling
- Local settings management

## 23. License

This project is licensed under the MIT License. See `LICENSE`.
