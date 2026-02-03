import Foundation

public enum WaveServiceContext: Sendable {
    case local
    case grpc(address: String)
    case remote(endpoint: URL, token: String)
}

public struct WaveServiceFactory {
    public static func create(for context: WaveServiceContext) -> any WaveServiceProtocol {
        switch context {
        case .local, .grpc, .remote:
            return LocalWaveService()
        }
    }
}
