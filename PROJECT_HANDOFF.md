# Project Handoff & Session State

**Date / Time**: 2026-11-23  
**Repository**: `https://github.com/suryaprakashrachamalla/aifeista.git`  
**Current Branch**: `main` (fully in sync with `origin/main`)  
**Active Project Directory**: `/Users/syedahmedzeshaan/Desktop/final-year-project`

---

## 1. Executive Summary & Repo Overview

This repository is a final-year project containing three core components:
1. **`pointer-base/`**: An AI desktop buddy for macOS based on HeyClicky / Learning Buddy. It sees the user's screens, listens to push-to-talk voice (`Ctrl + Option`), talks back, and uses a screen-pointer cursor to fly to and highlight UI elements across monitors.
2. **`base-harness/`**: Evaluation and computer-use agent harness.
3. **`controlbase-harness/`**: Control base harness imported from origin.

All git branches (`backup-before-sync`, `integrate-clicky`) were audited. The complete Clicky codebase from `integrate-clicky` was safely merged into `main`, renamed to `pointer-base/`, and pushed to `origin/main`. `pointer-base` matches git status cleanly.

---

## 2. Key Architecture Decisions & Code Changes Made

### A. Voice & Audio (100% On-Device Apple Native)
All external, paid, or API-key-dependent voice services (ElevenLabs TTS, AssemblyAI S2T, OpenAI Whisper) were **completely removed**:
- **Speech-to-Text (S2T)**: Uses Apple's native on-device dictation engine via `AppleSpeechTranscriptionProvider.swift` (`Speech.framework` + `AVAudioEngine`). Runs offline, free, and with zero latency.
- **Text-to-Speech (T2S)**: Uses Apple's native `AVSpeechSynthesizer` via `AppleTTSClient.swift`.
  - **Siri Neural Voice Support**: Dynamically prioritizes Siri voices and Apple Premium/Enhanced voices over compact legacy voices.
  - **Default Accent**: Pre-configured for **Samantha** (the original natural voice of Siri on macOS / iOS), providing fluid, coherent sentence flow, with automatic dynamic upgrade to downloaded Siri neural voices.
  - **Sentence Flow & Prosody**: Includes text sanitization that strips Markdown asterisks, backticks, code blocks, and URLs so the synthesizer speaks in coherent, fluid sentences instead of robotic word-by-word reading.
  - **Pacing**: Conversational speech rate (`0.48`) with natural sentence-boundary delays (`preUtteranceDelay: 0.05`, `postUtteranceDelay: 0.1`).

### B. LLM & Vision Backend (Groq via Local Cloudflare Worker)
- **Local Proxy Worker**: Located in `pointer-base/worker/`. Listens on `http://localhost:8787` and routes requests from `ClaudeAPI.swift` to Groq with Anthropic-to-OpenAI payload translation and SSE event streaming.
- **Configured Model**: `qwen/qwen3.8-27b` (verified active on Groq).
- **Environment Variables**: Stored safely in `pointer-base/worker/.dev.vars` (which is in `.gitignore`):
  ```ini
  LLM_PROVIDER=groq
  GROQ_API_KEY=gsk_...
  GROQ_MODEL=qwen/qwen3.8-27b
  ```
- **Obsolete routes removed**: `/tts` and `/transcribe-token` routes were deleted from `pointer-base/worker/src/index.ts`. Only `/chat` is maintained.

---

## 3. macOS Permissions & Code Signing Gotchas

When building Clicky ad-hoc with `CODE_SIGN_IDENTITY="-"`:
- **Designated Requirement**: The app binary in `~/Applications/Clicky.app` is signed with:
  ```bash
  codesign -f -s - --deep --entitlements pointer-base/leanring-buddy/leanring-buddy.entitlements -r='designated => identifier "com.yourcompany.leanring-buddy"' ~/Applications/Clicky.app
  ```
  This prevents macOS from treating each recompile as a new app with a changed `cdhash`.
- **Permanent Binary Location**: `~/Applications/Clicky.app`.
- **Required macOS Permissions**:
  - **Microphone**: For push-to-talk audio.
  - **Accessibility**: For global `Control + Option` shortcut detection and window bounds tracking.
  - **Screen Recording**: For `ScreenCaptureKit` / `CGPreflightScreenCaptureAccess` context.
  *(Note: macOS requires quitting and reopening the app after toggling Screen Recording for it to take effect).*

---

## 4. How to Run Locally

### 1. Start the Backend Worker (Port 8787)
```bash
cd pointer-base/worker
npm install
npx wrangler dev --port 8787
```

### 2. Build & Run the Mac App
- **From Xcode**: Open `pointer-base/leanring-buddy.xcodeproj`, select target `leanring-buddy`, press `Cmd + R`.
- **From Terminal**:
  ```bash
  xcodebuild -project pointer-base/leanring-buddy.xcodeproj -scheme leanring-buddy -configuration Debug CODE_SIGN_IDENTITY="-" build
  cp -R ~/Library/Developer/Xcode/DerivedData/leanring-buddy-*/Build/Products/Debug/Clicky.app ~/Applications/
  codesign -f -s - --deep --entitlements pointer-base/leanring-buddy/leanring-buddy.entitlements -r='designated => identifier "com.yourcompany.leanring-buddy"' ~/Applications/Clicky.app
  open ~/Applications/Clicky.app
  ```

---

## 5. Next Steps / Current State

- **App Status**: Built, installed in `~/Applications/Clicky.app`, tested with Groq.
- **Git Status**: Clean working tree on `main`, up to date with `origin/main` at commit `3a9c983d`.
- **Voice Recommendation**: If the user wants even higher quality Siri neural voices, they can download them in **System Settings $\rightarrow$ Accessibility $\rightarrow$ Spoken Content $\rightarrow$ Manage Voices...**.
