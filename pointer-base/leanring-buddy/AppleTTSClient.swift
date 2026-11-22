//
//  AppleTTSClient.swift
//  leanring-buddy
//
//  Text-to-speech using macOS's built-in AVSpeechSynthesizer.
//  Free, unlimited, works offline. Drop-in replacement for ElevenLabsTTSClient.
//

import AVFoundation
import Foundation

@MainActor
final class AppleTTSClient: NSObject {

    // The speech synthesizer is long-lived — creating one per utterance causes audio glitches.
    private let synthesizer = AVSpeechSynthesizer()

    // Indian English male voice (Rishi / en-IN), falls back to any en-IN or default English voice.
    private let preferredVoice: AVSpeechSynthesisVoice? = {
        let inVoices = AVSpeechSynthesisVoice.speechVoices()
            .filter { $0.language == "en-IN" }

        return inVoices.first(where: { $0.gender == .male || $0.name.lowercased().contains("rishi") })
            ?? inVoices.first
            ?? AVSpeechSynthesisVoice(language: "en-IN")
            ?? AVSpeechSynthesisVoice(language: "en-US")
    }()

    override init() {
        super.init()
        // Delegate lets us track finish events if needed in the future.
        synthesizer.delegate = self
    }

    /// Speaks `text` aloud using the system TTS engine.
    /// Returns as soon as speech begins — callers should poll `isPlaying` to detect completion.
    func speakText(_ text: String) async throws {
        // Stop anything already playing before starting the new utterance.
        synthesizer.stopSpeaking(at: .immediate)

        let utterance = AVSpeechUtterance(string: text)
        utterance.rate   = AVSpeechUtteranceDefaultSpeechRate
        utterance.pitchMultiplier = 1.0  // Natural male pitch
        utterance.volume = 1.0
        utterance.voice  = preferredVoice

        synthesizer.speak(utterance)
        print("🔊 Apple TTS: speaking \(text.count) characters via \(preferredVoice?.name ?? "default voice") (\(preferredVoice?.language ?? "en-IN"))")
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
