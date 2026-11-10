import Foundation
import Loopflow
let args = CommandLine.arguments
let messages: [ServerClientMessage] = [
    .hello(ServerHello(revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision, client: ServerClientIdentity(kind: "qa", platform: "macos", name: "LOO-790 QA", version: "branch"))),
    .request(id: "swift-task", operation: .read(.taskGet(directory: args[1], issue: args[2])), select: ["task.title"]),
    .subscribe(id: "swift-watch", operation: .taskGet(directory: args[1], issue: args[2]), select: ["row.condition.local_progress.dirty"], resume: nil),
]
for message in messages {
    let data = try JSONEncoder().encode(message)
    print(String(decoding: data, as: UTF8.self))
}
