import Foundation

public enum WaveServiceContext: Sendable {
    case local
    case grpc(address: String)
    case remote(endpoint: URL, token: String)
}

public struct WaveServiceFactory {
    public init() {}

    public static func create(for context: WaveServiceContext) -> any WaveServiceProtocol {
        switch context {
        case .local:
            return LocalWaveService()
        case .grpc:
            return LocalWaveService()
        case .remote:
            return LocalWaveService()
        }
    }
}
