import Foundation

public struct ConcertoConfig: Codable {
    public var connection: RemoteConnectionConfig? = nil
    public var container: ContainerConfig? = nil
}

public struct ContainerConfig: Codable {
    public var mounts: [ExtraMount]? = nil
    public var image: String? = nil
}

public struct ExtraMount: Codable, Equatable {
    public var path: String
    public var readOnly: Bool
}

public struct RemoteConnectionConfig: Codable {
    public var host: String
    public var port: Int

    public func toServerConnection() -> ServerConnection {
        ServerConnection(
            host: host,
            port: port,
            useTLS: true,
            authMode: .staticToken
        )
    }

    public var isLoopbackHost: Bool {
        let normalized = host.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1"
    }
}

extension ConcertoConfig {
    public var seededConnection: ServerConnection? {
        guard let connection, !connection.isLoopbackHost else {
            return nil
        }
        return connection.toServerConnection()
    }
}

public func loadConcertoConfig(configURL: URL? = nil) -> ConcertoConfig? {
    let resolvedURL = configURL ?? ConcertoConfig.defaultURL
    guard
        let raw = try? String(contentsOf: resolvedURL, encoding: .utf8),
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
    return ConcertoConfig(
        connection: parseConnectionConfig(lines),
        container: parseContainerConfig(lines)
    )
}

private func parseConnectionConfig(_ lines: [String]) -> RemoteConnectionConfig? {
    guard let section = extractTopLevelSection(named: "connection", lines: lines) else {
        return nil
    }

    var host: String?
    var port: Int?

    for line in section {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty || trimmed.hasPrefix("#") {
            continue
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
        return nil
    }

    return RemoteConnectionConfig(host: host, port: port)
}

private func parseContainerConfig(_ lines: [String]) -> ContainerConfig? {
    guard let section = extractTopLevelSection(named: "container", lines: lines) else {
        return nil
    }

    var image: String?
    var mounts: [ExtraMount] = []
    var parsingMounts = false
    var mountsIndent = 0

    for line in section {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty || trimmed.hasPrefix("#") {
            continue
        }

        if parsingMounts {
            let lineIndent = indentation(of: line)
            if lineIndent <= mountsIndent {
                parsingMounts = false
            } else if trimmed.hasPrefix("- ") {
                let rawSpec = String(trimmed.dropFirst(2))
                if let mount = parseExtraMount(rawSpec) {
                    mounts.append(mount)
                }
                continue
            }
        }

        guard let (key, value) = keyValuePair(from: trimmed) else {
            continue
        }

        switch key {
        case "image":
            let parsed = unquote(value)
            image = parsed.isEmpty ? nil : parsed
            parsingMounts = false
        case "mounts":
            parsingMounts = true
            mountsIndent = indentation(of: line)
        default:
            break
        }
    }

    if image == nil && mounts.isEmpty {
        return nil
    }

    return ContainerConfig(
        mounts: mounts.isEmpty ? nil : mounts,
        image: image
    )
}

private func parseExtraMount(_ raw: String) -> ExtraMount? {
    let trimmed = unquote(raw.trimmingCharacters(in: .whitespaces))
    guard !trimmed.isEmpty else {
        return nil
    }

    if let modeSeparator = trimmed.lastIndex(of: ":") {
        let modeStart = trimmed.index(after: modeSeparator)
        let mode = trimmed[modeStart...].lowercased()
        if mode == "ro" || mode == "rw" {
            let path = String(trimmed[..<modeSeparator]).trimmingCharacters(in: .whitespaces)
            guard !path.isEmpty else {
                return nil
            }
            return ExtraMount(path: path, readOnly: mode == "ro")
        }
    }

    return ExtraMount(path: trimmed, readOnly: true)
}

private func extractTopLevelSection(named sectionName: String, lines: [String]) -> [String]? {
    guard let sectionIndex = lines.firstIndex(where: {
        indentation(of: $0) == 0 &&
        $0.trimmingCharacters(in: .whitespaces) == "\(sectionName):"
    }) else {
        return nil
    }

    let sectionIndent = indentation(of: lines[sectionIndex])
    let sectionStart = lines.index(after: sectionIndex)
    guard sectionStart < lines.endIndex else {
        return []
    }

    var sectionLines: [String] = []
    for line in lines[sectionStart...] {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            sectionLines.append(line)
            continue
        }
        if indentation(of: line) <= sectionIndent {
            break
        }
        sectionLines.append(line)
    }

    return sectionLines
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
