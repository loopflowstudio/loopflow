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

    var isLocalhost: Bool {
        let normalized = host.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1"
    }
}

extension ConcertoConfig {
    var seededConnection: ServerConnection? {
        guard let connection, !connection.isLocalhost else {
            return nil
        }
        return connection.toServerConnection()
    }
}

func loadConcertoConfig(configURL: URL = ConcertoConfig.defaultURL) -> ConcertoConfig? {
    guard
        let raw = try? String(contentsOf: configURL, encoding: .utf8),
        !raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    else {
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
    let lines = raw.components(separatedBy: .newlines)
    guard let connectionIndex = lines.firstIndex(where: {
        $0.trimmingCharacters(in: .whitespaces) == "connection:"
    }) else {
        return ConcertoConfig(connection: nil)
    }

    let connectionIndent = indentation(of: lines[connectionIndex])
    var host: String?
    var port: Int?

    for line in lines[(connectionIndex + 1)...] {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty || trimmed.hasPrefix("#") {
            continue
        }

        if indentation(of: line) <= connectionIndent {
            break
        }

        guard let (key, value) = keyValuePair(from: trimmed) else {
            continue
        }

        switch key {
        case "host":
            host = unquote(value)
        case "port":
            port = Int(unquote(value))
        default:
            break
        }
    }

    guard let host, let port else {
        return ConcertoConfig(connection: nil)
    }

    return ConcertoConfig(connection: RemoteConnectionConfig(host: host, port: port))
}

private func indentation(of line: String) -> Int {
    line.prefix { $0 == " " || $0 == "\t" }.count
}

private func keyValuePair(from line: String) -> (String, String)? {
    guard let separatorIndex = line.firstIndex(of: ":") else {
        return nil
    }

    let key = line[..<separatorIndex].trimmingCharacters(in: .whitespaces)
    let valueStart = line.index(after: separatorIndex)
    let value = line[valueStart...].trimmingCharacters(in: .whitespaces)
    return (String(key), String(value))
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
