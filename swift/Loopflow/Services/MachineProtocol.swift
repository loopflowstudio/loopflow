import Foundation

/// Revision 1 of the Machine application protocol; carriers add no Work authority.
public struct MachineRequest: Encodable, Sendable {
    public let version: Int
    public let id: String
    public let operation: MachineOperation

    public init(version: Int, id: String, operation: MachineOperation) {
        self.version = version
        self.id = id
        self.operation = operation
    }
}

public enum MachineOperation: Encodable, Sendable {
    case bootstrap
    case sessionOwner(session: String)
    case read(ServedRead)
    case subscribe(ServedRead, generation: UInt64)
    case command(ServedWrite)
    case attach(owner: String)

    private enum Keys: String, CodingKey { case kind, query, generation, change, owner, session }
    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: Keys.self)
        switch self {
        case .sessionOwner(let session):
            try box.encode("session_owner", forKey: .kind)
            try box.encode(session, forKey: .session)
        case .bootstrap: try box.encode("bootstrap", forKey: .kind)
        case .read(let query):
            try box.encode("read", forKey: .kind)
            try box.encode(query, forKey: .query)
        case .subscribe(let query, let generation):
            try box.encode("subscribe", forKey: .kind)
            try box.encode(query, forKey: .query)
            try box.encode(generation, forKey: .generation)
        case .command(let change):
            try box.encode("command", forKey: .kind)
            try box.encode(change, forKey: .change)
        case .attach(let owner):
            try box.encode("attach", forKey: .kind)
            try box.encode(owner, forKey: .owner)
        }
    }
}

public struct MachineResponse: Decodable, Sendable {
    public let id: String
    public let instance: String
    public let result: MachineReply
}

public enum MachineReply: Decodable, Sendable {
    case bootstrap(version: Int, build: String, executableSHA256: String, machine: JSONValue, capabilities: [String], maxFrame: Int, maxConnections: Int)
    case read(String)
    case reading(generation: UInt64, frame: JSONValue)
    case command(traceID: String, output: String, stderr: String, code: Int)
    case sessionOwner(owner: String?)
    case bridged
    case error(code: String, message: String)

    private enum Keys: String, CodingKey {
        case kind, version, build, machine, capabilities, output, generation, frame, stderr, code, message, owner
        case traceID = "trace_id"
        case maxFrame = "max_frame", maxConnections = "max_connections", executableSHA256 = "executable_sha256"
    }
    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        switch try box.decode(String.self, forKey: .kind) {
        case "bootstrap":
            self = .bootstrap(version: try box.decode(Int.self, forKey: .version), build: try box.decode(String.self, forKey: .build),
                              executableSHA256: try box.decode(String.self, forKey: .executableSHA256),
                              machine: try box.decode(JSONValue.self, forKey: .machine), capabilities: try box.decode([String].self, forKey: .capabilities),
                              maxFrame: try box.decode(Int.self, forKey: .maxFrame), maxConnections: try box.decode(Int.self, forKey: .maxConnections))
        case "read": self = .read(try box.decode(String.self, forKey: .output))
        case "reading": self = .reading(generation: try box.decode(UInt64.self, forKey: .generation), frame: try box.decode(JSONValue.self, forKey: .frame))
        case "command": self = .command(traceID: try box.decode(String.self, forKey: .traceID), output: try box.decode(String.self, forKey: .output), stderr: try box.decode(String.self, forKey: .stderr), code: try box.decode(Int.self, forKey: .code))
        case "session_owner": self = .sessionOwner(owner: try box.decodeIfPresent(String.self, forKey: .owner))
        case "bridged": self = .bridged
        case "error": self = .error(code: try box.decode(String.self, forKey: .code), message: try box.decode(String.self, forKey: .message))
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: box, debugDescription: "Unsupported Machine response; update the client")
        }
    }
}
