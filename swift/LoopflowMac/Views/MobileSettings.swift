import AppKit
import CoreImage.CIFilterBuiltins
import CoreTransferable
import Loopflow
import SwiftUI
import UniformTypeIdentifiers

struct MobileSettings: View {
    @State private var setup = MobileSetup()
    @State private var name = "iPhone"
    @State private var removing: MobileSetup.Device?
    @Environment(\.palette) private var palette

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Take your work with you").font(Typography.sectionTitle())
                Text("Open Tasks and continue their Sessions from Loopflow on your iPhone. Sign in to Tailscale on both devices first.")
                    .foregroundStyle(palette.textSecondary)
                if let configuration = setup.configuration, configuration.enabled {
                    Label("Private mobile access is enabled", systemImage: "network")
                    Text(configuration.endpoint).font(.caption).textSelection(.enabled)
                        .foregroundStyle(palette.textSecondary)
                }
                if let pending = setup.pending {
                    pairing(pending)
                } else {
                    HStack {
                        TextField("Phone name", text: $name)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("mobile-phone-name")
                        Button("Pair a phone") { setup.pair(name: name) }
                            .buttonStyle(.borderedProminent)
                            .tint(palette.accent).foregroundStyle(palette.accentText)
                            .disabled(setup.busy || name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || name.utf8.count > 128)
                            .accessibilityIdentifier("mobile-pair")
                    }
                    Text("Loopflow detects Tailscale and prepares a private connection automatically.")
                        .font(.caption).foregroundStyle(palette.textSecondary)
                }
                if let activity = setup.activity { ProgressView(activity).controlSize(.small) }
                if let notice = setup.notice {
                    Label(notice, systemImage: "checkmark.circle").foregroundStyle(palette.available)
                }
                if let error = setup.error {
                    Text(error).foregroundStyle(palette.failure).textSelection(.enabled)
                }
                Divider()
                HStack {
                    Text("Paired phones").font(Typography.strong(14))
                    Spacer()
                    Button("Refresh", systemImage: "arrow.clockwise") { setup.refresh() }
                        .disabled(setup.busy)
                }
                if setup.loaded && setup.devices.isEmpty {
                    Text("No phones paired yet.").foregroundStyle(palette.textSecondary)
                }
                ForEach(setup.devices) { device in
                    HStack {
                        Image(systemName: "iphone")
                        VStack(alignment: .leading, spacing: 3) {
                            Text(device.name)
                            Text("Paired \(Date(timeIntervalSince1970: TimeInterval(device.createdAt)), style: .date)")
                                .font(.caption).foregroundStyle(palette.textSecondary)
                        }
                        Spacer()
                        Button("Remove access", role: .destructive) { removing = device }
                            .disabled(setup.busy)
                    }
                }
                if setup.configuration?.enabled == true {
                    Divider()
                    Button("Turn off mobile access") { setup.disable() }.disabled(setup.busy)
                    Text("Disconnects phones. Tasks and Sessions keep running. Pair a phone again to re-enable access; existing phone grants are retained.")
                        .font(.caption).foregroundStyle(palette.textSecondary)
                }
            }
            .padding(24)
        }
        .frame(minWidth: 560, idealWidth: 620, minHeight: 500)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { setup.refresh() }
        .onDisappear { setup.close() }
        .task(id: setup.pending?.id) { await setup.waitForPairing() }
        .confirmationDialog("Remove access for \(removing?.name ?? "this phone")?", isPresented: Binding(
            get: { removing != nil }, set: { if !$0 { removing = nil } }
        ), titleVisibility: .visible) {
            if let removing {
                Button("Remove access", role: .destructive) { setup.revoke(removing); self.removing = nil }
            }
        } message: {
            Text("The phone will disconnect and need a new pairing code. Its Tasks and Sessions keep running.")
        }
    }

    private func pairing(_ pending: MobileSetup.Pending) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Scan with Loopflow on your iPhone").font(Typography.strong(16))
            Image(nsImage: pending.image).interpolation(.none).resizable()
                .frame(width: 256, height: 256)
                .accessibilityLabel("One-use pairing code for \(pending.name)")
            Text("In the iPhone app, choose Scan pairing code, then Connect Machine.")
            Text("Expires \(pending.expires, style: .relative) from now. Keep this code private.")
                .font(.caption).foregroundStyle(palette.textSecondary)
            HStack {
                ShareLink(item: pending.transfer, preview: SharePreview("Loopflow phone setup")) {
                    Label("Share setup file", systemImage: "square.and.arrow.up")
                }
                Button("Hide code") { setup.pending = nil }
            }
        }
    }
}

@MainActor @Observable
final class MobileSetup {
    struct Configuration: Decodable {
        let enabled: Bool
        let port: UInt16
        let endpoint: String
    }

    struct Device: Decodable, Identifiable {
        let id: String
        let name: String
        let createdAt: Int64
        let revokedAt: Int64?
        enum CodingKeys: String, CodingKey {
            case id, name
            case createdAt = "created_at", revokedAt = "revoked_at"
        }
    }

    struct Pending {
        let id = UUID()
        let name: String
        let image: NSImage
        let expires: Date
        let knownDevices: Set<String>
        let transfer: SetupTransfer
    }

    struct SetupTransfer: Transferable {
        let data: Data
        static var transferRepresentation: some TransferRepresentation {
            DataRepresentation(exportedContentType: .json) { $0.data }
                .suggestedFileName("Loopflow setup.json")
        }
    }

    var configuration: Configuration?
    var devices: [Device] = []
    var pending: Pending?
    var activity: String?
    var busy: Bool { activity != nil }
    var loaded = false
    var error: String?
    var notice: String?
    private var operation: Task<Void, Never>?

    func refresh() { perform("Checking mobile access…") { try await self.read() } }

    func pair(name: String) {
        perform("Preparing a pairing code…") {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("loopflow-pair-\(UUID().uuidString)")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                attributes: [.posixPermissions: 0o700])
            defer { try? FileManager.default.removeItem(at: directory) }
            let output = directory.appendingPathComponent("setup.json")
            let started = Date()
            let phoneName = name.trimmingCharacters(in: .whitespacesAndNewlines)
            _ = try await Self.command(["pair", "--name", phoneName, "--output", output.path])
            let data = try Data(contentsOf: output)
            try FileManager.default.removeItem(at: output)
            _ = try MachinePairingTicket.decode(data)
            let image = try Self.qrCode(data)
            // Take the known-device set before displaying a code anyone can scan.
            try await self.read()
            self.pending = Pending(name: phoneName, image: image, expires: started.addingTimeInterval(600),
                knownDevices: Set(self.devices.map(\.id)), transfer: SetupTransfer(data: data))
        }
    }

    func disable() {
        pending = nil
        perform("Turning off mobile access…") {
            _ = try await Self.command(["mobile", "--off"])
            try await self.read()
            self.notice = "Mobile access is off. Your Sessions are still running."
        }
    }

    func revoke(_ device: Device) {
        perform("Removing phone access…") {
            _ = try await Self.command(["revoke", device.id])
            try await self.read()
            self.notice = "Access removed for \(device.name)."
        }
    }

    func close() {
        pending = nil
        operation?.cancel()
    }

    func waitForPairing() async {
        guard let pending else { return }
        do {
            while self.pending?.id == pending.id {
                try await Task.sleep(for: .seconds(3))
                if Date() >= pending.expires {
                    self.pending = nil
                    notice = "The pairing code expired. Choose Pair a phone for a new code."
                    return
                }
                let current = try await Self.readDevices()
                try Task.checkCancellation()
                guard self.pending?.id == pending.id else { return }
                devices = current
                if current.contains(where: { !pending.knownDevices.contains($0.id) && $0.name == pending.name }) {
                    self.pending = nil
                    notice = "\(pending.name) is paired. Open a Task on your phone to continue."
                }
            }
        } catch is CancellationError { }
        catch {
            // Stop showing a code when its use can no longer be observed.
            if self.pending?.id == pending.id {
                self.pending = nil
                self.error = "Couldn’t check pairing. Refresh the phone list before creating another code."
            }
        }
    }

    private func read() async throws {
        let status = try await Self.command(["mobile", "--status"])
        configuration = try JSONDecoder().decode(Configuration?.self, from: Data(status.utf8))
        devices = try await Self.readDevices()
        loaded = true
    }

    private static func readDevices() async throws -> [Device] {
        let output = try await command(["devices"])
        return try JSONDecoder().decode([Device].self, from: Data(output.utf8)).filter { $0.revokedAt == nil }
    }

    private static func command(_ arguments: [String]) async throws -> String {
        try await LocalWaveAgentLauncher.queryLf(["machine", "service"] + arguments, cwd: "/")
    }

    private func perform(_ activity: String, _ body: @escaping @MainActor () async throws -> Void) {
        guard !busy else { return }
        self.activity = activity
        error = nil
        notice = nil
        operation = Task {
            defer { self.activity = nil; operation = nil }
            do { try await body() }
            catch is CancellationError { }
            catch let failure as RegistryQueryError { error = failure.message }
            catch { self.error = error.localizedDescription }
        }
    }

    static func qrCode(_ data: Data) throws -> NSImage {
        let generator = CIFilter.qrCodeGenerator()
        generator.message = data
        generator.correctionLevel = "M"
        guard let code = generator.outputImage else { throw RegistryQueryError("Could not create the pairing code") }
        // Four white modules on every side keep the code readable in any theme.
        let bounds = code.extent.insetBy(dx: -4, dy: -4)
        let background = CIImage(color: .white).cropped(to: bounds)
        let padded = code.composited(over: background).transformed(by: CGAffineTransform(scaleX: 5, y: 5))
        guard let image = CIContext().createCGImage(padded, from: padded.extent) else {
            throw RegistryQueryError("Could not draw the pairing code")
        }
        return NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
    }
}
