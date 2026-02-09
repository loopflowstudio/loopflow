// LoopflowCore - Shared models and services for Concerto and Symphonia

import Foundation

// MARK: - Configuration

/// Default port for lfd HTTP server
public let lfdDefaultPort = 2486

/// Base URL for lfd HTTP server
public let lfdBaseURL = URL(string: "http://127.0.0.1:\(lfdDefaultPort)")!

/// Base URL for lfd API
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")

// Re-export all public types
// Models and services are imported directly from their source files
