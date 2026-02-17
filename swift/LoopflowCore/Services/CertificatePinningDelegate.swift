import CryptoKit
import Foundation
import Security

final class CertificatePinningDelegate: NSObject, @unchecked Sendable {
    private let connection: ServerConnection
    private let pinStore: CertificatePinStore

    private let stateQueue = DispatchQueue(label: "studio.loopflow.pin.delegate")
    private var trustRequirement: TrustRequirement?

    init(connection: ServerConnection, pinStore: CertificatePinStore) {
        self.connection = connection
        self.pinStore = pinStore
    }

    func consumeTrustRequirement() -> TrustRequirement? {
        stateQueue.sync {
            defer { trustRequirement = nil }
            return trustRequirement
        }
    }

    private func setTrustRequirement(_ requirement: TrustRequirement) {
        stateQueue.sync {
            trustRequirement = requirement
        }
    }

    private func fingerprint(for certificate: SecCertificate) -> String {
        let certData = SecCertificateCopyData(certificate) as Data
        let digest = SHA256.hash(data: certData)
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

extension CertificatePinningDelegate: URLSessionDelegate, URLSessionTaskDelegate {
    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        guard connection.useTLS,
              challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              let trust = challenge.protectionSpace.serverTrust,
              let certificateChain = SecTrustCopyCertificateChain(trust) as? [SecCertificate],
              let certificate = certificateChain.first
        else {
            completionHandler(.performDefaultHandling, nil)
            return
        }

        let newFingerprint = fingerprint(for: certificate)

        if let existingFingerprint = pinStore.pinnedFingerprint(for: connection) {
            if existingFingerprint == newFingerprint {
                completionHandler(.useCredential, URLCredential(trust: trust))
                return
            }

            setTrustRequirement(
                .certificateChanged(
                    host: connection.host,
                    port: connection.port,
                    oldFingerprint: existingFingerprint,
                    newFingerprint: newFingerprint
                )
            )
            completionHandler(.cancelAuthenticationChallenge, nil)
            return
        }

        pinStore.setPinnedFingerprint(newFingerprint, for: connection)
        completionHandler(.useCredential, URLCredential(trust: trust))
    }
}
