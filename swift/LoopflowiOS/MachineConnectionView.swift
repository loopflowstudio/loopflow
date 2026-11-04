import Loopflow
import SwiftUI

struct MachineConnectionView: View {
    @State private var savedMachine: SavedMachine?
    @State private var endpoint: URL?
    @State private var address = ""
    @State private var enteringAddress = false
    @FocusState private var addressFocused: Bool
    @State private var connecting = false
    @State private var scanning = false
    @State private var error: String?
    @Environment(\.colorScheme) private var colorScheme
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    var body: some View {
        Group {
            if let savedMachine {
                MobileTaskList(client: RemoteMobileClient(savedMachine: savedMachine),
                    connectionLabel: savedMachine.endpoint.host ?? "Machine", onForget: forget)
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
                                Label("Sign in to the same Tailscale account on both devices.", systemImage: "1.circle")
                                Label("On your Mac, open Loopflow → Settings → Mobile.", systemImage: "2.circle")
                                Label("Enable mobile access, then scan its code here.", systemImage: "3.circle")
                            }
                            .font(Typography.body(15))
                            if let endpoint {
                                Label(endpoint.host ?? "Machine", systemImage: "desktopcomputer")
                                    .font(Typography.strong(16))
                                Text(endpoint.absoluteString)
                                    .font(Typography.caption(13))
                                    .foregroundStyle(theme.palette.textSecondary)
                                    .textSelection(.enabled)
                                Button {
                                    Task { await connect(endpoint) }
                                } label: {
                                    HStack {
                                        if connecting { ProgressView() }
                                        Text(connecting ? "Connecting…" : "Connect Machine")
                                    }.frame(maxWidth: .infinity, minHeight: 44)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(theme.palette.accent)
                                .foregroundStyle(theme.palette.accentText)
                                .disabled(connecting)
                                Button("Scan another code") { scanning = true }.disabled(connecting)
                            } else {
                                Button { scanning = true } label: {
                                    Label("Scan QR code", systemImage: "qrcode.viewfinder")
                                        .frame(maxWidth: .infinity, minHeight: 44)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(theme.palette.accent)
                                .foregroundStyle(theme.palette.accentText)
                            }
                            if enteringAddress {
                                TextField("machine.tailnet.ts.net", text: $address)
                                    .textContentType(.URL)
                                    .keyboardType(.URL)
                                    .textInputAutocapitalization(.never)
                                    .autocorrectionDisabled()
                                    .textFieldStyle(.roundedBorder)
                                    .focused($addressFocused)
                                    .submitLabel(.continue)
                                    .onSubmit { accept(address) }
                                    .accessibilityLabel("Machine address")
                                Button("Continue") { accept(address) }
                                    .disabled(connecting || address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            } else {
                                Button("Enter address") {
                                    address = endpoint?.absoluteString ?? address
                                    enteringAddress = true
                                    addressFocused = true
                                }
                                    .frame(minHeight: 44)
                                    .disabled(connecting)
                            }
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
            MachineCodeScanner(accept: { scanned in
                endpoint = scanned
                enteringAddress = false
                addressFocused = false
                error = nil
                scanning = false
            }, enterAddress: {
                scanning = false
                enteringAddress = true
                addressFocused = true
            })
        }
        .task {
            do { savedMachine = try MachineKeychain.load() }
            catch { self.error = error.localizedDescription }
        }
        .alert("Machine connection", isPresented: Binding(
            get: { savedMachine != nil && error != nil },
            set: { if !$0 { error = nil } }
        )) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }

    private func accept(_ text: String) {
        do {
            endpoint = try MachineAddress.decode(text)
            addressFocused = false
            enteringAddress = false
            error = nil
        } catch {
            endpoint = nil
            self.error = "Enter the Tailscale address shown in Desktop’s Mobile settings."
        }
    }

    private func connect(_ endpoint: URL) async {
        connecting = true
        error = nil
        defer { connecting = false }
        do { savedMachine = try await RemoteMobileClient.connect(endpoint: endpoint) }
        catch let error as RegistryQueryError { self.error = error.message }
        catch { self.error = "Couldn’t connect. Check that both devices use the same Tailscale account and mobile access is enabled on the Machine." }
    }

    private func forget() {
        do {
            try MachineKeychain.forget()
            savedMachine = nil
            endpoint = nil
        } catch { self.error = error.localizedDescription }
    }
}
