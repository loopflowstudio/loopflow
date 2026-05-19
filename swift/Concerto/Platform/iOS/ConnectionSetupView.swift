#if os(iOS)
import AVFoundation
import SwiftUI
import LoopflowCore

struct ConnectionSetupView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette

    @State private var pastedLink = ""
    @State private var errorMessage: String?
    @State private var isConnecting = false
    @State private var showingScanner = false
    @State private var showingStudio = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: Spacing.lg) {
                    hero
                    pasteCard
                    studioCard

                    if let errorMessage {
                        Text(errorMessage)
                            .font(Typography.caption())
                            .foregroundStyle(Color.statusError)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .accessibilityLabel("Connection error: \(errorMessage)")
                    }
                }
                .padding(Spacing.lg)
            }
            .navigationTitle("Connect to lfd")
            .sheet(isPresented: $showingScanner) {
                PairingScannerView { url in
                    showingScanner = false
                    connect(url)
                }
            }
            .sheet(isPresented: $showingStudio) {
                DiscoveryView()
            }
        }
    }

    private var hero: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Image(systemName: "qrcode.viewfinder")
                .font(.system(size: 44))
                .foregroundStyle(Color.loopflowBurgundy)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Scan a pairing QR")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(Color.loopflowBurgundy)
                Text("Run `lf op pair` on your host, then scan the QR or paste the link below.")
                    .font(Typography.body())
                    .foregroundStyle(.secondary)
            }

            Button {
                showingScanner = true
            } label: {
                Label("Scan QR", systemImage: "camera.viewfinder")
                    .frame(maxWidth: .infinity, minHeight: HitTarget.touch)
            }
            .buttonStyle(.borderedProminent)
            .disabled(isConnecting)
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private var pasteCard: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Paste pairing link")
                .font(Typography.sectionTitle())
                .foregroundStyle(Color.loopflowBurgundy)

            TextField("loopflow://pair?host=…", text: $pastedLink, axis: .vertical)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .font(Typography.code(13))
                .lineLimit(2...4)
                .padding(Spacing.md)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .accessibilityLabel("Pairing link")

            Button {
                guard let url = URL(string: pastedLink.trimmingCharacters(in: .whitespacesAndNewlines)) else {
                    errorMessage = "Paste a valid loopflow://pair link."
                    return
                }
                connect(url)
            } label: {
                if isConnecting {
                    ProgressView()
                        .frame(maxWidth: .infinity, minHeight: HitTarget.touch)
                } else {
                    Text("Connect")
                        .frame(maxWidth: .infinity, minHeight: HitTarget.touch)
                }
            }
            .buttonStyle(.bordered)
            .disabled(isConnecting || pastedLink.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private var studioCard: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Sign in with Loopflow")
                .font(Typography.sectionTitle())
                .foregroundStyle(Color.loopflowBurgundy)
            Text("Use studio discovery if your daemon is registered with loopflow.studio.")
                .font(Typography.body())
                .foregroundStyle(.secondary)
            Button {
                showingStudio = true
            } label: {
                Label("Sign in", systemImage: "person.crop.circle.badge.checkmark")
                    .frame(maxWidth: .infinity, minHeight: HitTarget.touch)
            }
            .buttonStyle(.bordered)
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func connect(_ url: URL) {
        isConnecting = true
        errorMessage = nil
        Task {
            do {
                try await repoState.connect(pairingURL: url, outputBuffer: outputBuffer)
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                }
            }
            await MainActor.run {
                isConnecting = false
            }
        }
    }
}

private struct PairingScannerView: UIViewControllerRepresentable {
    let onCode: (URL) -> Void

    func makeUIViewController(context: Context) -> ScannerViewController {
        let controller = ScannerViewController()
        controller.onCode = onCode
        return controller
    }

    func updateUIViewController(_ uiViewController: ScannerViewController, context: Context) {}
}

private final class ScannerViewController: UIViewController, @preconcurrency AVCaptureMetadataOutputObjectsDelegate {
    var onCode: ((URL) -> Void)?
    private let session = AVCaptureSession()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configure()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if !session.isRunning {
            DispatchQueue.global(qos: .userInitiated).async { [session] in
                session.startRunning()
            }
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if session.isRunning {
            session.stopRunning()
        }
    }

    private func configure() {
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else {
            showMessage("Camera unavailable. Paste the pairing link instead.")
            return
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            showMessage("QR scanning unavailable. Paste the pairing link instead.")
            return
        }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]

        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.bounds
        view.layer.addSublayer(preview)
    }

    private func showMessage(_ message: String) {
        let label = UILabel()
        label.text = message
        label.textColor = .white
        label.textAlignment = .center
        label.numberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            label.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
              let value = object.stringValue,
              let url = URL(string: value) else {
            return
        }
        session.stopRunning()
        onCode?(url)
    }
}
#endif
