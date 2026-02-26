import Foundation

struct ConcertoConfig: Codable {
    var connection: RemoteConnectionConfig?
}

struct RemoteConnectionConfig: Codable {
    var host: String
    var port: Int

    func toServerConnection() -> ServerConnection {
        ServerConnection(
            host: host,
            port: port,
            useTLS: true,
            authMode: .staticToken
        )
    }
}

func loadConcertoConfig(configURL: URL = ConcertoConfig.defaultURL) -> ConcertoConfig? {
    guard let raw = try? String(contentsOf: configURL, encoding: .utf8) else {
        return nil
    }

    if raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        return nil
    }

    return parseConcertoConfig(raw)
}

private extension ConcertoConfig {
    static var defaultURL: URL {
        URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
            .appendingPathComponent(".lf", isDirectory: true)
            .appendingPathComponent("concerto.yaml", isDirectory: false)
    }
}

private func parseConcertoConfig(_ raw: String) -> ConcertoConfig {
    var host: String?
    var port: Int?
    var inConnection = false
    var connectionIndent = 0

    for line in raw.components(separatedBy: .newlines) {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty || trimmed.hasPrefix("#") {
            continue
        }

        let indentation = line.prefix { $0 == " " || $0 == "\t" }.count

        if !inConnection {
            if trimmed == "connection:" {
                inConnection = true
                connectionIndent = indentation
            }
            continue
        }

        if indentation <= connectionIndent {
            break
        }

        guard let separatorIndex = trimmed.firstIndex(of: ":") else {
            continue
        }

        let key = String(trimmed[..<separatorIndex]).trimmingCharacters(in: .whitespaces)
        let valueStart = trimmed.index(after: separatorIndex)
        let value = String(trimmed[valueStart...]).trimmingCharacters(in: .whitespaces)
        let normalizedValue = unquote(value)

        switch key {
        case "host":
            host = normalizedValue
        case "port":
            port = Int(normalizedValue)
        default:
            continue
        }
    }

    if let host, let port {
        return ConcertoConfig(connection: RemoteConnectionConfig(host: host, port: port))
    }
    return ConcertoConfig(connection: nil)
}

private func unquote(_ value: String) -> String {
    guard value.count >= 2 else {
        return value
    }

    if value.hasPrefix("\""), value.hasSuffix("\"") {
        return String(value.dropFirst().dropLast())
    }

    if value.hasPrefix("'"), value.hasSuffix("'") {
        return String(value.dropFirst().dropLast())
    }

    return value
}
