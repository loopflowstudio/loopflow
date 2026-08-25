#if os(macOS)
import Foundation
import Loopflow

enum LaunchTargetLauncher {
    struct Command: Equatable {
        let cwd: String
        let argv: [String]
        let environment: [String: String]
    }

    static func isRemoteHome(_ route: String) -> Bool {
        let trimmed = route.trimmingCharacters(in: .whitespaces)
        return !trimmed.isEmpty
            && trimmed != "localhost"
            && trimmed != "local"
            && !trimmed.hasSuffix("@local")
            && trimmed != "127.0.0.1"
            && trimmed != "::1"
    }

    static func command(for session: AskSessionRecord, localCwd: String) -> Command {
        guard isRemoteHome(session.homeRoute) else {
            return Command(cwd: localCwd, argv: session.attachArgv, environment: [:])
        }

        let remoteCommand = "exec " + session.attachArgv.map(shellQuote).joined(separator: " ")
        return Command(
            cwd: "/",
            argv: sshArgv(
                host: sshHost(from: session.homeRoute),
                home: session.homeRoute,
                remoteCommand: remoteCommand
            ),
            environment: [:]
        )
    }

    private static func shellQuote(_ value: String) -> String {
        "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private static func sshArgv(host: String, home: String?, remoteCommand: String) -> [String] {
        let (hostname, port) = splitHostAndPort(host)
        let destination: String
        if let home,
           home.hasPrefix("ssh://"),
           let at = home.dropFirst("ssh://".count).firstIndex(of: "@") {
            let owner = home.dropFirst("ssh://".count)[..<at]
            destination = "\(owner)@\(hostname)"
        } else {
            destination = hostname
        }
        var argv = ["ssh"]
        if let port {
            argv.append(contentsOf: ["-p", port])
        }
        argv.append(contentsOf: [destination, remoteCommand])
        return argv
    }

    private static func sshHost(from homeRoute: String) -> String {
        guard homeRoute.hasPrefix("ssh://") else { return homeRoute }
        let authority = homeRoute.dropFirst("ssh://".count)
        return authority.split(separator: "@", maxSplits: 1).last.map(String.init)
            ?? String(authority)
    }

    private static func splitHostAndPort(_ host: String) -> (String, String?) {
        if host.hasPrefix("["), let bracket = host.firstIndex(of: "]") {
            let destination = String(host[host.index(after: host.startIndex) ..< bracket])
            let suffix = host[host.index(after: bracket)...]
            if suffix.hasPrefix(":"), suffix.dropFirst().allSatisfy(\.isNumber) {
                return (destination, String(suffix.dropFirst()))
            }
            return (destination, nil)
        }
        guard host.filter({ $0 == ":" }).count == 1,
              let colon = host.lastIndex(of: ":"),
              host[host.index(after: colon)...].allSatisfy(\.isNumber)
        else { return (host, nil) }
        return (
            String(host[..<colon]),
            String(host[host.index(after: colon)...])
        )
    }
}
#endif
