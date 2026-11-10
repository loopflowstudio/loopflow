import Foundation
import Loopflow

let args = CommandLine.arguments
let encoder = JSONEncoder()
if args[1] == "encode" {
    let read = ServedRead.taskGet(directory: args[2], issue: args[3])
    let messages: [ServerClientMessage] = [
        .hello(ServerHello(revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision,
                           client: ServerClientIdentity(kind: "qa", platform: "macos", name: "Swift live QA", version: "1"))),
        .subscribe(id: "swift-watch", operation: read, select: nil, resume: nil),
        .request(id: "swift-read", operation: .read(read), select: nil),
    ]
    for message in messages {
        print(String(decoding: try encoder.encode(message), as: UTF8.self))
    }
} else {
    var readings = 0
    var steers = 0
    var steps = 0
    for line in try String(contentsOfFile: args[2], encoding: .utf8).split(separator: "\n") {
        let entry = try JSONSerialization.jsonObject(with: Data(line.utf8)) as! [String: Any]
        guard entry["kind"] as? String == "receive" else { continue }
        let data = try JSONSerialization.data(withJSONObject: entry["value"]!)
        switch try JSONDecoder().decode(ServerMessage.self, from: data) {
        case .reading: readings += 1
        case .event(_, _, .taskSteer(_, let steer, let comment)):
            precondition(!steer.id.isEmpty && comment != nil)
            steers += 1
        case .event(_, _, .graphStepFinished): steps += 1
        default: break
        }
    }
    precondition(readings > 0 && steers > 0 && steps == 2)
    print("Swift decoded \(readings) live readings, \(steers) steer events, and \(steps) Graph step events.")
}
