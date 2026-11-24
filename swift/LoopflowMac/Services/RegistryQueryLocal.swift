// Native Work reads, writes and subscriptions use the local server, which
// routes a Task's to the Machine that runs it. Execution launches keep the CLI.

#if os(macOS)
import Foundation
import Loopflow

enum RegistryQueryLocal {
    static let shared = RegistryQuery(runWithInput: { args, cwd, input in
        try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd, input: input)
    }, serve: { query in
        try await ServerClient.read(query)
    }, write: { change in
        switch change {
        case .taskCreate(_, let requestID, _, _, _, _, _, _, _), .sessionNew(_, let requestID, _, _):
            let answer = try await CreationSubmissions.shared.submit(change, requestID: requestID)
            do {
                switch change {
                case .taskCreate: _ = try RegistryQuery.decode(TaskCreated.self, from: answer)
                case .sessionNew: _ = try RegistryQuery.decode(SessionRecord.self, from: answer)
                default: break
                }
                return answer
            } catch { throw SubmittedCreationError(requestID: requestID, cause: error) }
        default: return try await ServerClient.write(change)
        }
    }, change: { change, requestID in
        try await ServerClient.change(change, requestID: requestID)
    }, subscribeRead: { read in
        ServerClient.subscribe(read)
    }, subscribeWork: {
        try WorkSubscriptions.observe { ServerClient.subscribe($0) }
    }) { args, cwd in
        try await Perf.measure(Perf.lf, args.prefix(2).joined(separator: " ")) {
            let started = ContinuousClock.now
            var ok = false
            defer { LaunchJournal.home.read(args, ms: started.elapsedMs, ok: ok) }
            let output = try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
            ok = true
            return output
        }
    }
}

#endif
