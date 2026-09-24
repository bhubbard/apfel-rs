// ============================================================================
// foundation_models_bridge.swift — C ABI bridge to macOS FoundationModels
// Part of apfel-rs — High-performance Rust fork of apfel
// ============================================================================

import Foundation
import FoundationModels

public typealias BridgeStreamCallback = @convention(c) (
    UnsafePointer<CChar>?, // chunk (delta text)
    Bool,                  // is_done
    UnsafePointer<CChar>?, // finish_reason ("stop", "length", "guardrail", etc.)
    UnsafePointer<CChar>?, // error message (nil if no error)
    UnsafeMutableRawPointer? // user_data context pointer
) -> Void

// MARK: - Prewarmed Model Singletons

private let defaultModel = SystemLanguageModel(guardrails: .default)
private let permissiveModel = SystemLanguageModel(guardrails: .permissiveContentTransformations)

// MARK: - Model Inspection

@_cdecl("apfel_bridge_is_available")
public func apfel_bridge_is_available() -> Bool {
    return defaultModel.isAvailable
}

@_cdecl("apfel_bridge_context_size")
public func apfel_bridge_context_size() -> Int32 {
    let size = defaultModel.contextSize
    return Int32(size > 0 ? size : 4096)
}

@_cdecl("apfel_bridge_token_count")
public func apfel_bridge_token_count(text: UnsafePointer<CChar>) -> Int32 {
    let str = String(cString: text)
    guard !str.isEmpty else { return 0 }
    if #available(macOS 26.4, *) {
        let sema = DispatchSemaphore(value: 0)
        var count = Int32(max(1, str.count / 4))
        Task {
            do {
                let n = try await defaultModel.tokenCount(for: str)
                count = Int32(n)
            } catch {
                count = Int32(max(1, str.count / 4))
            }
            sema.signal()
        }
        sema.wait()
        return count
    } else {
        return Int32(max(1, str.count / 4))
    }
}

private var cachedLanguagesString: String? = nil

@_cdecl("apfel_bridge_supported_languages")
public func apfel_bridge_supported_languages() -> UnsafePointer<CChar>? {
    if let cached = cachedLanguagesString {
        return (cached as NSString).utf8String
    }
    var seen = Set<String>()
    var ids: [String] = []
    for language in defaultModel.supportedLanguages {
        if let id = language.languageCode?.identifier, seen.insert(id).inserted {
            ids.append(id)
        }
    }
    let joined = ids.isEmpty ? "en" : ids.joined(separator: ", ")
    cachedLanguagesString = joined
    return (joined as NSString).utf8String
}

// MARK: - Generation Request Schema

struct BridgeMessagePayload: Decodable {
    let role: String
    let content: String?
}

struct BridgeGeneratePayload: Decodable {
    let prompt: String?
    let system_prompt: String?
    let messages: [BridgeMessagePayload]?
    let temperature: Double?
    let top_p: Double?
    let max_tokens: Int?
    let seed: UInt64?
    let permissive: Bool?
}

// MARK: - Generation Execution

@_cdecl("apfel_bridge_generate_json")
public func apfel_bridge_generate_json(
    requestJSON: UnsafePointer<CChar>,
    userData: UnsafeMutableRawPointer?,
    callback: BridgeStreamCallback
) -> Int32 {
    let jsonStr = String(cString: requestJSON)
    guard let data = jsonStr.data(using: .utf8),
          let req = try? JSONDecoder().decode(BridgeGeneratePayload.self, from: data) else {
        let err = "Invalid JSON payload sent to FoundationModels bridge"
        err.withCString { callback(nil, true, nil, $0, userData) }
        return 2 // Usage error
    }
    
    // Resolve final prompt
    var finalPrompt = req.prompt ?? ""
    var transcriptEntries: [Transcript.Entry] = []
    
    // System prompt instructions
    var instructionsText = req.system_prompt ?? ""
    if let messages = req.messages {
        for msg in messages {
            if msg.role == "system" || msg.role == "developer" {
                if let c = msg.content, !c.isEmpty {
                    if !instructionsText.isEmpty {
                        instructionsText += "\n\n"
                    }
                    instructionsText += c
                }
            }
        }
    }
    
    if !instructionsText.isEmpty {
        let segment = Transcript.TextSegment(content: instructionsText)
        let instructions = Transcript.Instructions(segments: [.text(segment)], toolDefinitions: [])
        transcriptEntries.append(.instructions(instructions))
    }
    
    // Process message history if provided
    if let messages = req.messages {
        let conv = messages.filter { $0.role != "system" && $0.role != "developer" }
        if !conv.isEmpty {
            for (idx, msg) in conv.enumerated() {
                let content = msg.content ?? ""
                if idx == conv.count - 1 && finalPrompt.isEmpty {
                    finalPrompt = content
                } else {
                    if msg.role == "user" {
                        let seg = Transcript.TextSegment(content: content)
                        let promptEntry = Transcript.Prompt(segments: [.text(seg)])
                        transcriptEntries.append(.prompt(promptEntry))
                    } else if msg.role == "assistant" {
                        let seg = Transcript.TextSegment(content: content)
                        let respEntry = Transcript.Response(assetIDs: [], segments: [.text(seg)])
                        transcriptEntries.append(.response(respEntry))
                    }
                }
            }
        }
    }
    
    if finalPrompt.isEmpty {
        let err = "No prompt provided for generation"
        err.withCString { callback(nil, true, nil, $0, userData) }
        return 2
    }
    
    var completed = false
    var exitCode: Int32 = 0
    
    let thread = Thread {
        let runLoop = RunLoop.current
        Task {
            do {
                let permissive = req.permissive ?? false
                let model = permissive ? permissiveModel : defaultModel
                
                let session: LanguageModelSession
                if !transcriptEntries.isEmpty {
                    session = LanguageModelSession(model: model, transcript: Transcript(entries: transcriptEntries))
                } else {
                    session = LanguageModelSession(model: model)
                }
                
                // Generation options
                var options = GenerationOptions()
                options.temperature = req.temperature
                if let maxTokens = req.max_tokens {
                    options.maximumResponseTokens = maxTokens
                }
                if let topP = req.top_p {
                    options.sampling = .random(probabilityThreshold: topP, seed: req.seed)
                } else if req.temperature == 0.0 {
                    options.sampling = .greedy
                }
                
                let stream = session.streamResponse(to: finalPrompt, options: options)
                var prevLen = 0
                for try await snapshot in stream {
                    let content = snapshot.content
                    if content.count > prevLen {
                        let deltaIndex = content.index(content.startIndex, offsetBy: prevLen)
                        let delta = String(content[deltaIndex...])
                        delta.withCString { cStr in
                            callback(cStr, false, nil, nil, userData)
                        }
                        prevLen = content.count
                    }
                }
                
                let finishReason = (req.max_tokens != nil && prevLen >= (req.max_tokens! * 3)) ? "length" : "stop"
                finishReason.withCString { reason in
                    callback(nil, true, reason, nil, userData)
                }
                exitCode = 0
            } catch {
                let errString = "\(error)"
                let reason: String
                let code: Int32
                if errString.lowercased().contains("guardrail") || errString.lowercased().contains("safety") {
                    reason = "guardrail"
                    code = 3
                } else if errString.lowercased().contains("context") || errString.lowercased().contains("token") {
                    reason = "context_overflow"
                    code = 4
                } else {
                    reason = "error"
                    code = 1
                }
                errString.withCString { err in
                    reason.withCString { r in
                        callback(nil, true, r, err, userData)
                    }
                }
                exitCode = code
            }
            completed = true
        }
        
        while !completed {
            runLoop.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02))
        }
    }
    
    thread.start()
    while !completed {
        Thread.sleep(forTimeInterval: 0.01)
    }
    
    return exitCode
}
