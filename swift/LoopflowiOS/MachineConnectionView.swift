import Loopflow
import SwiftUI
import UniformTypeIdentifiers

struct MachineConnectionView: View {
    @State private var credential: MachineCredential?
    @State private var ticket: MachinePairingTicket?
    @State private var pairing = false
    @State private var importing = false
    @State private var scanning = false
    @State private var error: String?
    @Environment(\.colorScheme) private var colorScheme
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    var body: some View {
        Group {
            if let credential {
                MobileTaskList(client: RemoteMobileClient(credential: credential),
                    connectionLabel: credential.endpoint.host ?? "Paired Machine", onForget: forget)
            } else {
                NavigationStack {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 24) {
                            HStack(spacing: 12) {
                                Image(systemName: "desktopcomputer")
                                Image(systemName: "arrow.left.arrow.right").font(.system(size: 16))
                                Image(systemName: "iphone")
                            }
                            .font(.system(size: 34, weight: .light))
                            .foregroundStyle(theme.palette.accentInk)
                            .accessibilityHidden(true)
                            Text("Your work,\nwithin reach.")
                                .font(Typography.display)
                            Text("Connect to your Loopflow Machine to open Tasks and continue their Sessions.")
                                .font(Typography.body(17))
                                .foregroundStyle(theme.palette.textSecondary)
                            VStack(alignment: .leading, spacing: 14) {
                                Label("Sign in to Tailscale on both devices.", systemImage: "1.circle")
                                Label("On your Mac, open Loopflow → Settings → Mobile.", systemImage: "2.circle")
                                Label("Choose Pair a phone, then scan its code here.", systemImage: "3.circle")
                            }
                            .font(Typography.body(15))
                            if let ticket {
                                Label(ticket.endpoint.host ?? "Machine", systemImage: "checkmark.circle")
                                    .font(Typography.strong(16))
                                Button {
                                    Task { await pair(ticket) }
                                } label: {
                                    HStack {
                                        if pairing { ProgressView() }
                                        Text(pairing ? "Connecting…" : "Connect Machine")
                                    }.frame(maxWidth: .infinity, minHeight: 44)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(theme.palette.accent)
                                .foregroundStyle(theme.palette.accentText)
                                .disabled(pairing)
                                Button("Scan another code") { scanning = true }.disabled(pairing)
                            } else {
                                Button { scanning = true } label: {
                                    Label("Scan pairing code", systemImage: "qrcode.viewfinder")
                                        .frame(maxWidth: .infinity, minHeight: 44)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(theme.palette.accent)
                                .foregroundStyle(theme.palette.accentText)
                            }
                            DisclosureGroup("Other ways to connect") {
                                Text("In Desktop’s Mobile settings, choose Share setup file.")
                                    .font(Typography.caption(13))
                                    .foregroundStyle(theme.palette.textSecondary)
                                    .padding(.top, 8)
                                HStack(spacing: 16) {
                                    Button("Import setup file", systemImage: "doc") { importing = true }
                                    PasteButton(payloadType: String.self) { values in
                                        if let value = values.first { accept(Data(value.utf8)) }
                                    }
                                }
                                .padding(.top, 8)
                            }
                            .disabled(pairing)
                            if let error {
                                Text(error)
                                    .font(Typography.body(14))
                                    .foregroundStyle(theme.palette.textSecondary)
                                    .accessibilityLabel("Connection issue: \(error)")
                            }
                            Text("Your Sessions run on your Machine. This phone connects through your private Tailscale network.")
                                .font(Typography.caption(13))
                                .foregroundStyle(theme.palette.textSecondary)
                        }
                        .padding(28)
                    }
                    .background(theme.palette.background)
                    .navigationTitle("Loopflow")
                    .navigationBarTitleDisplayMode(.inline)
                }
            }
        }
        .tint(theme.palette.accentInk)
        .foregroundStyle(theme.palette.text)
        .sheet(isPresented: $scanning) {
            MachineCodeScanner { scanned in
                ticket = scanned
                error = nil
                scanning = false
            }
        }
        .task {
            do { credential = try MachineKeychain.load() }
            catch { self.error = error.localizedDescription }
        }
        .alert("Machine connection", isPresented: Binding(
            get: { credential != nil && error != nil },
            set: { if !$0 { error = nil } }
        )) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
            do {
                let url = try result.get()
                let granted = url.startAccessingSecurityScopedResource()
                defer { if granted { url.stopAccessingSecurityScopedResource() } }
                let file = try FileHandle(forReadingFrom: url)
                defer { try? file.close() }
                accept(try file.read(upToCount: 4097) ?? Data())
            } catch { self.error = "The setup file could not be read. Import the file created by your Machine." }
        }
    }

    private func accept(_ data: Data) {
        do {
            ticket = try MachinePairingTicket.decode(data)
            error = nil
        } catch {
            ticket = nil
            self.error = "This is not a valid Loopflow setup ticket. Scan a new code or import the file created by your Machine."
        }
    }

    private func pair(_ ticket: MachinePairingTicket) async {
        pairing = true
        error = nil
        defer { pairing = false; self.ticket = nil }
        do { credential = try await MachinePairing.redeem(ticket) }
        catch let error as RegistryQueryError { self.error = error.message }
        catch { self.error = "Couldn’t connect. Check that both devices are on Tailscale and the Machine service is running. A ticket expires after ten minutes and can be used once." }
    }

    private func forget() {
        do {
            try MachineKeychain.forget()
            credential = nil
        } catch { self.error = error.localizedDescription }
    }
}
