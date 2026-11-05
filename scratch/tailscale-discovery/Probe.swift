// Research only: receive address announcements without reading Tailscale's
// LocalAPI from iOS. An announcement is a candidate, never authorization.
import Foundation
import Network
#if os(iOS)
import SwiftUI
#endif

struct Offer: Decodable {
    let version: Int
    let service: String
    let endpoint: String

    static func address(_ data: Data) -> URL? {
        guard data.count <= 2048,
              let offer = try? JSONDecoder().decode(Self.self, from: data),
              offer.version == 1, offer.service == "loopflow",
              let parts = URLComponents(string: offer.endpoint),
              parts.scheme == "https", parts.host?.hasSuffix(".ts.net") == true,
              parts.user == nil, parts.password == nil,
              parts.query == nil, parts.fragment == nil,
              parts.path.isEmpty || parts.path == "/" else { return nil }
        return parts.url
    }
}

final class Receiver {
    private var listener: NWListener?
    var changed: (String) -> Void = { _ in }
    var received: (URL) -> Void = { _ in }

    func start() {
        stop()
        do {
            let listener = try NWListener(using: .udp, on: 47471)
            self.listener = listener
            listener.stateUpdateHandler = { [weak self] state in
                switch state {
                case .ready: self?.changed("Listening on UDP 47471")
                case .failed(let error): self?.changed("Listener failed: \(error)")
                default: break
                }
            }
            listener.newConnectionHandler = { [weak self] connection in
                connection.start(queue: .main)
                connection.receiveMessage { data, _, _, error in
                    defer { connection.cancel() }
                    guard error == nil, let data, let address = Offer.address(data) else { return }
                    self?.received(address)
                }
            }
            listener.start(queue: .main)
        } catch { changed("Could not listen: \(error)") }
    }

    func stop() {
        listener?.stateUpdateHandler = nil
        listener?.newConnectionHandler = nil
        listener?.cancel()
        listener = nil
    }
}

#if os(iOS)
@main
struct ProbeApp: App {
    var body: some Scene { WindowGroup { ProbeView() } }
}

struct ProbeView: View {
    @State private var receiver = Receiver()
    @State private var status = "Preparing…"
    @State private var addresses: [URL] = []
    @Environment(\.scenePhase) private var phase

    var body: some View {
        NavigationStack {
            List {
                Section("Discovery experiment") {
                    Text(status)
                    Text("Sign into Tailscale, then keep this app open while the host sends announcements. No server address is entered on this device.")
                }
                Section("Unverified address offers") {
                    ForEach(addresses, id: \.self) { Text($0.absoluteString) }
                }
                Section {
                    Text("Receiving an offer does not authenticate a server. A production client must verify HTTPS, Loopflow identity and access before showing a Machine as available.")
                }
            }
            .navigationTitle("Machine discovery")
        }
        .onAppear {
            receiver.changed = { status = $0 }
            receiver.received = { if !addresses.contains($0) { addresses.append($0) } }
            receiver.start()
        }
        .onChange(of: phase) { _, value in
            if value == .active { receiver.start() }
            else { receiver.stop(); status = "Stopped while away" }
        }
        .onDisappear { receiver.stop() }
    }
}
#else
// Host-side parser/listener proof only. It is not iOS/tailnet acceptance.
@main
struct ProbeCLI {
    static func main() {
        let receiver = Receiver()
        receiver.changed = { print($0); fflush(stdout) }
        receiver.received = { _ in print("Received a valid address offer"); fflush(stdout); exit(0) }
        receiver.start()
        DispatchQueue.main.asyncAfter(deadline: .now() + 15) { print("No offer received"); exit(1) }
        RunLoop.main.run()
    }
}
#endif
