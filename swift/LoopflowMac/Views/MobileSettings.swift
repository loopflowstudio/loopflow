import AppKit
import CoreImage.CIFilterBuiltins
import Loopflow
import SwiftUI

struct MobileSettings: View {
    @State private var setup = MobileSetup()
    @Environment(\.palette) private var palette

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Take your work with you").font(Typography.sectionTitle())
                Text("Open Tasks and continue their Sessions from Loopflow on your iPhone. Sign in to the same Tailscale account on both devices.")
                    .foregroundStyle(palette.textSecondary)
                if let configuration = setup.configuration, configuration.enabled {
                    Label("Private mobile access is enabled", systemImage: "network")
                    if let image = setup.image {
                        Image(nsImage: image).interpolation(.none).resizable()
                            .frame(width: 256, height: 256)
                            .accessibilityLabel("Machine address QR code")
                    }
                    Text("In Loopflow on your iPhone, choose Scan QR code, then Connect Machine.")
                    Text(configuration.endpoint).font(.caption).textSelection(.enabled)
                        .foregroundStyle(palette.textSecondary)
                    HStack {
                        Button("Copy address", systemImage: "doc.on.doc") {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(configuration.endpoint, forType: .string)
                        }
                        ShareLink(item: configuration.endpoint) {
                            Label("Share address", systemImage: "square.and.arrow.up")
                        }
                    }
                    Text("This code contains only the address. Access requires your Tailscale account.")
                        .font(.caption).foregroundStyle(palette.textSecondary)
                    Divider()
                    Button("Turn off mobile access") { setup.disable() }.disabled(setup.busy)
                    Text("Disconnects phones. Tasks and Sessions keep running. Manage individual devices in Tailscale.")
                        .font(.caption).foregroundStyle(palette.textSecondary)
                } else {
                    Button("Enable mobile access") { setup.enable() }
                        .buttonStyle(.borderedProminent)
                        .tint(palette.accent).foregroundStyle(palette.accentText)
                        .disabled(setup.busy)
                        .accessibilityIdentifier("mobile-enable")
                    Text("Loopflow detects Tailscale and prepares a private connection automatically.")
                        .font(.caption).foregroundStyle(palette.textSecondary)
                }
                if let activity = setup.activity { ProgressView(activity).controlSize(.small) }
                if let error = setup.error {
                    Text(error).foregroundStyle(palette.failure).textSelection(.enabled)
                }
                Button("Refresh", systemImage: "arrow.clockwise") { setup.refresh() }.disabled(setup.busy)
            }
            .padding(24)
        }
        .frame(minWidth: 560, idealWidth: 620, minHeight: 500)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { setup.refresh() }
        .onDisappear { setup.close() }
    }
}

@MainActor @Observable
final class MobileSetup {
    typealias Configuration = MachineMobileConfiguration

    var configuration: Configuration?
    var image: NSImage?
    var activity: String?
    var busy: Bool { activity != nil }
    var error: String?
    private var operation: Task<Void, Never>?

    func refresh() { perform("Checking mobile access…") { try await self.read() } }

    func enable() {
        perform("Preparing mobile access…") {
            _ = try await Self.command(["mobile"])
            try await self.read()
        }
    }

    func disable() {
        perform("Turning off mobile access…") {
            _ = try await Self.command(["mobile", "--off"])
            try await self.read()
        }
    }

    func close() { operation?.cancel() }

    private func read() async throws {
        let status = try await Self.command(["mobile", "--status"])
        let value = try JSONDecoder().decode(Configuration?.self, from: Data(status.utf8))
        let code: NSImage?
        if let value, value.enabled {
            let address = try MachineAddress.decode(value.endpoint)
            code = try Self.qrCode(Data(address.absoluteString.utf8))
        } else { code = nil }
        try Task.checkCancellation()
        configuration = value
        image = code
    }

    private static func command(_ arguments: [String]) async throws -> String {
        try await LocalWaveAgentLauncher.queryLf(["machine", "service"] + arguments, cwd: "/")
    }

    private func perform(_ activity: String, _ body: @escaping @MainActor () async throws -> Void) {
        guard !busy else { return }
        self.activity = activity
        error = nil
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
        guard let code = generator.outputImage else { throw RegistryQueryError("Could not create the QR code") }
        // Four white modules on every side keep the code readable in any theme.
        let bounds = code.extent.insetBy(dx: -4, dy: -4)
        let background = CIImage(color: .white).cropped(to: bounds)
        let padded = code.composited(over: background).transformed(by: CGAffineTransform(scaleX: 5, y: 5))
        guard let image = CIContext().createCGImage(padded, from: padded.extent) else {
            throw RegistryQueryError("Could not draw the QR code")
        }
        return NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
    }
}
