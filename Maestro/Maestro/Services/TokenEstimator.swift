// Service for estimating token counts by shelling out to `lf`.

import Foundation

struct TokenEstimator {
    func estimate(
        prompt: String?,
        args: String,
        context: [URL],
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool,
        in repoURL: URL
    ) async -> Int {
        var cmdArgs = ["lf"]

        if let p = prompt, !p.isEmpty {
            cmdArgs.append(p)
        } else {
            cmdArgs.append(":")
        }

        if !args.isEmpty {
            cmdArgs.append(args)
        }

        for url in context {
            let relativePath = url.path().replacingOccurrences(of: repoURL.path() + "/", with: "")
            cmdArgs.append("-x")
            cmdArgs.append(relativePath)
        }

        // Include context flags to match actual command
        if includeDiff {
            cmdArgs.append("--diff")
        }
        if !includeDiffFiles {
            cmdArgs.append("--no-diff-files")
        }
        if !includeDocs {
            cmdArgs.append("--no-docs")
        }
        if includePaste {
            cmdArgs.append("--paste")
        }

        cmdArgs.append("-c")

        do {
            let output = try await run(cmdArgs, in: repoURL)
            return parseTokenCount(from: output)
        } catch {
            return 0
        }
    }

    private func run(_ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""
                continuation.resume(returning: output)
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    private func parseTokenCount(from output: String) -> Int {
        // Parse output like "Tokens: 12,340"
        // Or JSON output with token field
        if let data = output.data(using: .utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let tokens = json["tokens"] as? Int {
            return tokens
        }

        // Fallback: look for "Tokens: X" pattern
        let pattern = #"Tokens:\s*([\d,]+)"#
        if let regex = try? NSRegularExpression(pattern: pattern),
           let match = regex.firstMatch(in: output, range: NSRange(output.startIndex..., in: output)),
           let range = Range(match.range(at: 1), in: output) {
            let tokenString = output[range].replacingOccurrences(of: ",", with: "")
            return Int(tokenString) ?? 0
        }

        return 0
    }
}
