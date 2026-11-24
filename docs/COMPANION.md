# Carnival Companion (macOS Spatial & Voice Buddy)

Native macOS Swift companion (`companion/leanring-buddy/`) that provides push-to-talk voice (`Ctrl + Option`), on-device Apple Speech & Siri neural TTS, ScreenCaptureKit awareness, and a flying screen cursor (`[POINT:x,y:label]`) routed through Carnival's `:8787` bridge.

---

## 1. Voice & Audio (100% On-Device Apple Native)

- **Speech-to-Text (`AppleSpeechTranscriptionProvider.swift`)**: Uses Apple's on-device `Speech.framework` + `AVAudioEngine` with zero external API keys.
- **Text-to-Speech (`AppleTTSClient.swift`)**: Uses `AVSpeechSynthesizer` prioritizing Siri neural voices (with **Samantha** default) and sanitizes Markdown tokens for natural conversational pacing (`0.48` rate).

## 2. LLM & Vision Backend (`companion/worker/`)

- Listens on `http://localhost:8787` (standalone worker) or connects directly to `AI Carnival.app` / `carnival-daemon` on `http://127.0.0.1:8787/chat` with full SQLite WAL session persistence and spatial overlay tags.
- Local secrets belong in `companion/worker/.dev.vars` (gitignored).

## 3. Build & Run Companion App

```bash
xcodebuild -project companion/leanring-buddy.xcodeproj -scheme leanring-buddy -configuration Debug CODE_SIGN_IDENTITY="-" build
cp -R ~/Library/Developer/Xcode/DerivedData/leanring-buddy-*/Build/Products/Debug/Clicky.app ~/Applications/
codesign -f -s - --deep --entitlements companion/leanring-buddy/leanring-buddy.entitlements -r='designated => identifier "com.yourcompany.leanring-buddy"' ~/Applications/Clicky.app
open ~/Applications/Clicky.app
```

Grant **Microphone**, **Accessibility** (`Ctrl + Option` hotkey & window bounds), and **Screen Recording** permissions in **System Settings → Privacy & Security**.
