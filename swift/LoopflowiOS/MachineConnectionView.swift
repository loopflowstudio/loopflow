import Loopflow
import SwiftUI

struct MachineConnectionView: View {
    @State private var savedMachine: SavedMachine?
    @State private var changingAddress = false
    @State private var error: String?

    var body: some View {
        Group {
            if let savedMachine {
                MobileTaskList(client: RemoteMobileClient(savedMachine: savedMachine),
                    connectionLabel: savedMachine.endpoint.host ?? "Machine", onForget: forget,
                    onChangeAddress: { changingAddress = true })
                    .id(savedMachine.endpoint)
            } else {
                MachineAddressView(replacing: nil) { savedMachine = $0 }
            }
        }
        .sheet(isPresented: $changingAddress) {
            if let savedMachine {
                MachineAddressView(replacing: savedMachine) { replacement in
                    self.savedMachine = replacement
                    changingAddress = false
                }
            }
        }
        .task {
            do { savedMachine = try MachineKeychain.load() }
            catch { self.error = error.localizedDescription }
        }
        .alert("Machine connection", isPresented: Binding(
            get: { error != nil },
            set: { if !$0 { error = nil } }
        )) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }

    private func forget() {
        do {
            try MachineKeychain.forget()
            savedMachine = nil
        } catch { self.error = error.localizedDescription }
    }
}

struct MachineAddressView: View {
    let replacing: SavedMachine?
    let onConnected: (SavedMachine) -> Void
    @State private var endpoint: URL?
    @State private var address = ""
    @State private var enteringAddress = false
    @FocusState private var addressFocused: Bool
    @State private var connecting = false
    @State private var scanning = false
    @State private var error: String?
    @State private var operation: Task<Void, Never>?
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var colorScheme
    private var theme: ThemeDefinition { colorScheme == .dark ? .dark : .light }

    var body: some View {
        NavigationStack {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        if replacing == nil {
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
                        }
                        Text(replacing == nil
                            ? "Connect to your Loopflow Machine to open Tasks and continue their Sessions."
                            : "Scan the new code in Desktop’s Mobile settings. Your saved connection stays until the new address is verified.")
                            .font(Typography.body(17))
                            .foregroundStyle(theme.palette.textSecondary)
                        if let replacing {
                            Text(replacing.endpoint.absoluteString)
                                .font(Typography.caption(13))
                                .foregroundStyle(theme.palette.textSecondary)
                                .textSelection(.enabled)
                        } else {
                            VStack(alignment: .leading, spacing: 14) {
                                Label("Sign in to the same Tailscale account on both devices.", systemImage: "1.circle")
                                Label("On your Mac, open Loopflow → Settings → Mobile.", systemImage: "2.circle")
                                Label("Enable mobile access, then scan its code here.", systemImage: "3.circle")
                            }
                            .font(Typography.body(15))
                        }
                        if let endpoint {
                            Label(endpoint.host ?? "Machine", systemImage: "desktopcomputer")
                                .font(Typography.strong(16))
                            Text(endpoint.absoluteString)
                                .font(Typography.caption(13))
                                .foregroundStyle(theme.palette.textSecondary)
                                .textSelection(.enabled)
                            Button {
                                operation = Task { await connect(endpoint) }
                            } label: {
                                HStack {
                                    if connecting { ProgressView() }
                                    Text(connecting ? "Connecting…" : (replacing == nil ? "Connect Machine" : "Update address"))
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
                                .id("machine-connection-error")
                        }
                        Text("Your Sessions run on your Machine. This phone connects through your private Tailscale network.")
                            .font(Typography.caption(13))
                            .foregroundStyle(theme.palette.textSecondary)
                    }
                    .padding(28)
                }
                .background(theme.palette.background)
                .onChange(of: error) { _, value in
                    if value != nil {
                        withAnimation { proxy.scrollTo("machine-connection-error", anchor: .bottom) }
                    }
                }
            }
            .navigationTitle(replacing == nil ? "Loopflow" : "Change address")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if replacing != nil {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { operation?.cancel(); dismiss() }
                    }
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
        .interactiveDismissDisabled(connecting)
        .onDisappear { operation?.cancel() }
    }

    private func accept(_ text: String) {
        guard !connecting else { return }
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
        guard !connecting else { return }
        connecting = true
        error = nil
        defer { connecting = false; operation = nil }
        do {
            let connected = try await RemoteMobileClient.connect(endpoint: endpoint, replacing: replacing)
            onConnected(connected)
        } catch is CancellationError { }
        catch let error as RegistryQueryError { self.error = error.message }
        catch { self.error = "Couldn’t connect. Check that both devices use the same Tailscale account and mobile access is enabled on the Machine." }
    }
}
