//
//  AppleTTSClient.swift
//  leanring-buddy
//
//  Text-to-speech using macOS's built-in AVSpeechSynthesizer.
//  Prefers Siri and Neural Enhanced/Premium voices for natural sentence flow.
//

import AVFoundation
import Foundation

@MainActor
final class AppleTTSClient: NSObject {

    // The speech synthesizer is long-lived — creating one per utterance causes audio glitches.
    private let synthesizer = AVSpeechSynthesizer()

    /// Dynamically selects the most natural voice available on this Mac:
    /// 1. Siri Neural Voice (Voice 1, Voice 2, etc.)
    /// 2. Premium Quality Voice (e.g. Ava Premium, Samantha Premium)
    /// 3. Enhanced Quality Voice (e.g. Rishi Enhanced, Samantha Enhanced, Oliver)
    /// 4. Indian English (Rishi) or natural US English fallback
    private var preferredVoice: AVSpeechSynthesisVoice? {
        let allVoices = AVSpeechSynthesisVoice.speechVoices()

        // 1. Siri voices (highest priority — neural natural prosody)
        if let siriVoice = allVoices.first(where: {
            $0.language.hasPrefix("en") && (
                $0.name.localizedCaseInsensitiveContains("siri") ||
                $0.identifier.localizedCaseInsensitiveContains("siri")
            )
        }) {
            return siriVoice
        }

        // 2. Premium quality English voices
        if let premiumVoice = allVoices.first(where: {
            $0.quality == .premium && $0.language.hasPrefix("en")
        }) {
            return premiumVoice
        }

        // 3. Enhanced quality English voices (including Enhanced Rishi / Samantha)
        if let enhancedVoice = allVoices.first(where: {
            $0.quality == .enhanced && $0.language.hasPrefix("en")
        }) {
            return enhancedVoice
        }

        // 4. Natural fallback: Samantha (Siri's original voice, smooth and coherent)
        return allVoices.first(where: { $0.name.localizedCaseInsensitiveContains("samantha") })
            ?? AVSpeechSynthesisVoice(language: "en-US")
    }

    override init() {
        super.init()
        synthesizer.delegate = self
    }

    /// Sanitizes text before speaking so the engine reads natural sentences
    /// instead of stumbling over markdown symbols, backticks, or code blocks.
    private func sanitizeTextForSpeech(_ text: String) -> String {
        var cleaned = text
        // Remove code blocks and inline code
        cleaned = cleaned.replacingOccurrences(of: "```[\\s\\S]*?```", with: "", options: .regularExpression)
        cleaned = cleaned.replacingOccurrences(of: "`[^`]+`", with: "", options: .regularExpression)
        // Remove URLs
        cleaned = cleaned.replacingOccurrences(of: "https?://\\S+", with: "", options: .regularExpression)
        // Remove markdown formatting characters (*, _, #, ~, >)
        cleaned = cleaned.replacingOccurrences(of: "[*_~#>]+", with: "", options: .regularExpression)
        // Normalize bullet points to clean pauses
        cleaned = cleaned.replacingOccurrences(of: "^\\s*[-•*]\\s+", with: "", options: [.regularExpression, .anchored])
        // Normalize multiple spaces and newlines into clean sentence breaks
        cleaned = cleaned.replacingOccurrences(of: "\\n+", with: ". ", options: .regularExpression)
        cleaned = cleaned.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        return cleaned.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Speaks `text` aloud using the system TTS engine.
    /// Returns as soon as speech begins — callers should poll `isPlaying` to detect completion.
    func speakText(_ text: String) async throws {
        // Stop anything already playing before starting the new utterance.
        synthesizer.stopSpeaking(at: .immediate)

        let cleanedText = sanitizeTextForSpeech(text)
        guard !cleanedText.isEmpty else { return }

        let voice = preferredVoice
        let utterance = AVSpeechUtterance(string: cleanedText)

        // 0.48 rate gives the neural engine time to form natural sentence melody and breathing pauses
        utterance.rate = 0.48
        utterance.pitchMultiplier = 1.0
        utterance.volume = 1.0
        utterance.preUtteranceDelay = 0.05
        utterance.postUtteranceDelay = 0.1
        utterance.voice = voice

        synthesizer.speak(utterance)
        let qualityTag = voice?.quality == .premium ? "[Premium]" : (voice?.quality == .enhanced ? "[Enhanced]" : "[Compact]")
        print("🔊 Apple TTS: speaking via \(voice?.name ?? "default") (\(voice?.language ?? "en")) \(qualityTag)")
    }

    /// Whether TTS audio is currently playing back.
    var isPlaying: Bool {
        synthesizer.isSpeaking
    }

    /// Stops any in-progress playback immediately.
    func stopPlayback() {
        synthesizer.stopSpeaking(at: .immediate)
    }
}

// AVSpeechSynthesizerDelegate conformance — nonisolated because delegate callbacks
// arrive on an internal Speech framework queue, not necessarily the main actor.
extension AppleTTSClient: AVSpeechSynthesizerDelegate {
    nonisolated func speechSynthesizer(
        _ synthesizer: AVSpeechSynthesizer,
        didFinish utterance: AVSpeechUtterance
    ) {
        // No action needed — isPlaying (synthesizer.isSpeaking) already reflects completion.
    }
}
