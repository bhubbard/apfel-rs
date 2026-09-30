// ============================================================================
// eval_runner.swift — On-Device Accuracy and Performance Evaluation Harness
// Benchmarks candidate selection accuracy, latency, and prewarming on macOS 26+
// using SystemLanguageModel(.contentTagging)
// ============================================================================

import Foundation
import FoundationModels

struct Sample {
    let prompt: String
    let expected: String
}

@main
struct EvalRunner {
    static func main() async {
        print("=================================================================")
        print("Running FoundationModels .contentTagging Evaluation on macOS \(ProcessInfo.processInfo.operatingSystemVersionString)")
        print("=================================================================")

        let samples: [Sample] = [
            Sample(prompt: "I want to cancel my subscription and get a refund for last month's invoice.", expected: "billing"),
            Sample(prompt: "How much does the annual enterprise plan cost?", expected: "billing"),
            Sample(prompt: "My password reset email never arrives and I cannot log into my dashboard.", expected: "technical_support"),
            Sample(prompt: "The API keeps returning HTTP 504 Gateway Timeout during peak hours.", expected: "technical_support"),
            Sample(prompt: "I need to add two more team members to our organization account.", expected: "account_management"),
            Sample(prompt: "Can we transfer ownership of our workspace to a different admin email?", expected: "account_management"),
            Sample(prompt: "What is the recipe for chocolate chip cookies?", expected: "insufficient"),
            Sample(prompt: "Preheat the oven to 375 degrees and grease the baking sheet.", expected: "insufficient"),
        ]

        let model = SystemLanguageModel(useCase: .contentTagging, guardrails: .permissiveContentTransformations)
        let instructions = """
        You are a deterministic classification engine. Classify the input context into EXACTLY one category:
        - billing
        - technical_support
        - account_management
        - insufficient

        Respond ONLY with the category name inside brackets like [category].
        """

        let session = LanguageModelSession(model: model, instructions: instructions)
        let prewarmStart = DispatchTime.now()
        session.prewarm()
        let prewarmEnd = DispatchTime.now()
        let prewarmMs = Double(prewarmEnd.uptimeNanoseconds - prewarmStart.uptimeNanoseconds) / 1_000_000.0
        print(String(format: "Session prewarmed in %.2f ms\n", prewarmMs))

        var passedCount = 0
        var totalLatencyMs = 0.0

        for (index, sample) in samples.enumerated() {
            var options = GenerationOptions()
            options.temperature = 0.0
            options.sampling = .greedy
            options.maximumResponseTokens = 8

            let start = DispatchTime.now()
            do {
                let response = try await session.respond(to: sample.prompt, options: options)
                let end = DispatchTime.now()
                let latencyMs = Double(end.uptimeNanoseconds - start.uptimeNanoseconds) / 1_000_000.0
                totalLatencyMs += latencyMs

                let raw = response.content.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                let passed = raw.contains(sample.expected)
                if passed {
                    passedCount += 1
                    print(String(format: "  [%d/%d] ✅ [PASS] (%.1f ms) Expected: %-20s | Got: %s", index + 1, samples.count, latencyMs, sample.expected, raw))
                } else {
                    print(String(format: "  [%d/%d] ❌ [FAIL] (%.1f ms) Expected: %-20s | Got: %s", index + 1, samples.count, latencyMs, sample.expected, raw))
                }
            } catch {
                print(String(format: "  [%d/%d] ⚠️ [ERROR] Sample failed: %@", index + 1, samples.count, "\(error)"))
            }
        }

        let accuracy = Double(passedCount) / Double(samples.count) * 100.0
        let avgLatencyMs = totalLatencyMs / Double(samples.count)
        print("\n-----------------------------------------------------------------")
        print(String(format: "Summary: Accuracy = %.1f%% (%d/%d) | Avg Latency = %.2f ms", accuracy, passedCount, samples.count, avgLatencyMs))
        print("=================================================================")
    }
}
