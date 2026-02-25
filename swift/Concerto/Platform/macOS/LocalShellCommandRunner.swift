#if os(macOS)
import Foundation
import LoopflowCore

enum LocalShellCommandRunner {
    static let run: WaveService.ShellCommandRunner = { args in
        let cmd = args.joined(separator: " ")
        LoggingService.lfd("runShellCommand: \(cmd)")

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = ["-l", "-c", cmd]

            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

                LoggingService.lfd("runShellCommand: exit=\(process.terminationStatus) output=\(output.prefix(200))")

                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(
                        throwing: WaveServiceError.commandFailed(
                            output.isEmpty ? "Exit code \(process.terminationStatus)" : output
                        )
                    )
                }
            } catch {
                LoggingService.lfd("runShellCommand: exception=\(error.localizedDescription)")
                continuation.resume(throwing: error)
            }
        }
    }
}
#endif
