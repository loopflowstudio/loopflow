import AppKit
import CoreImage.CIFilterBuiltins
import Loopflow
import SwiftUI

struct MobileSettings: View {
    let setup: MobileSetup
    var highlightedKey: String?
    @Environment(\.palette) private var palette

    var body: some View {
        Form {
            Section {
                MobileAccessToggle(setup: setup)
                    .settingsRow(SettingsMatch.mobileAccess.id, highlighted: highlightedKey)
                Text("Sign in to the same Tailscale account on this Machine and your iPhone.")
                    .foregroundStyle(.secondary)
            }
            Section {
                if let configuration = setup.configuration, configuration.enabled {
                    Label(setup.ready ? "Ready to connect" : "Mobile connection unavailable",
                          systemImage: setup.ready ? "network" : "network.slash")
                    if let image = setup.image {
                        Image(nsImage: image).interpolation(.none).resizable()
                            .frame(width: 256, height: 256)
                            .accessibilityLabel("Machine address QR code")
                    }
                    if setup.ready {
                        Text("In Loopflow on your iPhone, choose Scan QR code, then Connect Machine.")
                    } else {
                        Text("Your Machine address is saved. Restore the connection, then refresh to show its QR code.")
                            .foregroundStyle(palette.textSecondary)
                        Button("Set up again") { Task { await setup.change(enabled: true) } }.disabled(setup.busy)
                        Text("Uses this Machine’s signed-in Tailscale account.")
                            .font(Typography.meta).foregroundStyle(palette.textSecondary)
                    }
                    Text(configuration.endpoint).font(Typography.mono).textSelection(.enabled)
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
                        .font(Typography.meta).foregroundStyle(palette.textSecondary)
                }
                if let activity = setup.activity { ProgressView(activity).controlSize(.small) }
                if let error = setup.error ?? setup.status?.error {
                    Text(error).foregroundStyle(palette.failure).textSelection(.enabled)
                }
                Button("Refresh", systemImage: "arrow.clockwise") { Task { await setup.refresh() } }.disabled(setup.busy)
            }
        }
        .settingsForm(highlighted: highlightedKey)
    }
}

struct MobileAccessToggle: View {
    let setup: MobileSetup

    var body: some View {
        Toggle(SettingsMatch.mobileAccess.title, isOn: Binding(get: { setup.configuration?.enabled == true }, set: { enabled in
            Task { await setup.change(enabled: enabled) }
        }))
        .toggleStyle(.switch)
        .disabled(setup.busy)
    }
}

@MainActor @Observable
final class MobileSetup {
    typealias Configuration = MachineMobileConfiguration

    private(set) var status: MachineMobileStatus?
    var configuration: Configuration? { status?.configuration }
    private(set) var image: NSImage?
    var ready: Bool { image != nil }
    private(set) var activity: String?
    var busy: Bool { activity != nil }
    private(set) var error: String?
    private let command: ([String]) async throws -> String

    init(command: @escaping ([String]) async throws -> String = { arguments in
        try await LocalWaveAgentLauncher.queryLf(["server"] + arguments, cwd: "/")
    }) {
        self.command = command
    }

    func observe() async {
        image = nil
        while !Task.isCancelled {
            await refresh()
            do { try await Task.sleep(for: .seconds(30)) } catch { return }
        }
    }

    func refresh() async {
        await perform("Checking mobile access…") { try await self.read() }
    }

    func change(enabled: Bool) async {
        guard !busy else { return }
        image = nil
        await perform(enabled ? "Preparing mobile access…" : "Turning off mobile access…") {
            do { _ = try await self.command(enabled ? ["mobile"] : ["mobile", "--off"]) }
            catch {
                try Task.checkCancellation()
                let failure = error
                // The command may have committed before losing its reply.
                // Recover the current state without replaying the command.
                do { try await self.read() }
                catch { throw failure }
                let confirmed = self.status?.error == nil && (enabled
                    ? self.ready && self.configuration?.enabled == true
                    : !self.ready && self.configuration?.enabled != true)
                guard confirmed else { throw failure }
                return
            }
            try await self.read()
        }
    }

    private func read() async throws {
        do {
            let data = try await self.command(["mobile", "--status"])
            let value = try JSONDecoder().decode(MachineMobileStatus.self, from: Data(data.utf8))
            let code: NSImage?
            if value.ready, let configuration = value.configuration, configuration.enabled {
                let address = try MachineAddress.decode(configuration.endpoint)
                code = try Self.qrCode(Data(address.absoluteString.utf8))
            } else { code = nil }
            try Task.checkCancellation()
            status = value
            image = code
        } catch {
            image = nil
            throw error
        }
    }

    private func perform(_ activity: String, _ body: () async throws -> Void) async {
        guard !busy else { return }
        self.activity = activity
        error = nil
        defer { self.activity = nil }
        do { try await body() }
        catch is CancellationError { }
        catch let failure as RegistryQueryError { error = failure.message }
        catch { self.error = error.localizedDescription }
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
