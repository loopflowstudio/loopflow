// Service for running tasks with embedded terminal support.

import Foundation
import SwiftUI

@MainActor
@Observable
final class TaskRunner {
    var isRunning = false
    var currentCommand: String?
    var currentWorkingDirectory: URL?

    private var onComplete: (() -> Void)?

    func startTask(command: String, workingDirectory: URL, onComplete: @escaping () -> Void) {
        self.currentCommand = command
        self.currentWorkingDirectory = workingDirectory
        self.onComplete = onComplete
        self.isRunning = true
    }

    func cancelTask() {
        isRunning = false
        currentCommand = nil
    }

    func handleTermination() {
        isRunning = false
        onComplete?()
        onComplete = nil
    }
}
