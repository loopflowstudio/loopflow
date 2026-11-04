import AVFoundation
import Loopflow
import SwiftUI
import VisionKit

struct MachineCodeScanner: View {
    let accept: (URL) -> Void
    let enterAddress: () -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.openURL) private var openURL
    @State private var available = false
    @State private var message: String?
    @State private var denied = false

    var body: some View {
        NavigationStack {
            Group {
                if available, scenePhase == .active {
                    CameraScanner(accept: accept, issue: { message = $0 })
                        .overlay(alignment: .bottom) {
                            Text(message ?? "Point your camera at the code in Loopflow’s Mobile settings.")
                                .font(Typography.body(16))
                                .padding(20).frame(maxWidth: .infinity)
                                .background(.regularMaterial)
                        }
                } else {
                    ContentUnavailableView {
                        Label("Scan a QR code", systemImage: "qrcode.viewfinder")
                    } description: {
                        Text(message ?? "Preparing the camera…")
                    } actions: {
                        if denied, let settings = URL(string: UIApplication.openSettingsURLString) {
                            Button("Open Settings") { openURL(settings) }
                        }
                        Button("Enter an address instead", action: enterAddress)
                    }
                }
            }
            .navigationTitle("Connect your Machine")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { available = false }
            }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                guard DataScannerViewController.isSupported else {
                    message = "This device can’t scan codes. Enter or paste its address on the previous screen."
                    return
                }
                let granted = await AVCaptureDevice.requestAccess(for: .video)
                guard !Task.isCancelled else { return }
                denied = !granted
                available = granted && DataScannerViewController.isAvailable
                if !available {
                    message = granted
                        ? "The camera is unavailable. You can still enter or paste its address."
                        : "Allow camera access in Settings, or enter its address without using the camera."
                }
            }
        }
    }
}

private struct CameraScanner: UIViewControllerRepresentable {
    let accept: (URL) -> Void
    let issue: (String) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(accept: accept, issue: issue) }

    func makeUIViewController(context: Context) -> ScannerController {
        let controller = ScannerController()
        controller.scanner.delegate = context.coordinator
        controller.issue = issue
        return controller
    }

    func updateUIViewController(_ controller: ScannerController, context: Context) {}

    static func dismantleUIViewController(_ controller: ScannerController, coordinator: Coordinator) {
        controller.scanner.stopScanning()
    }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let accept: (URL) -> Void
        let issue: (String) -> Void
        private var accepted = false

        init(accept: @escaping (URL) -> Void, issue: @escaping (String) -> Void) {
            self.accept = accept
            self.issue = issue
        }

        func dataScanner(_ scanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
            for item in addedItems {
                guard !accepted, case .barcode(let barcode) = item,
                      let text = barcode.payloadStringValue else { continue }
                guard let address = try? MachineAddress.decode(text) else {
                    issue("Use the QR code in Loopflow’s Mobile settings.")
                    continue
                }
                accepted = true
                scanner.stopScanning()
                accept(address)
            }
        }

        func dataScanner(_ scanner: DataScannerViewController, becameUnavailableWithError error: DataScannerViewController.ScanningUnavailable) {
            scanner.stopScanning()
            issue("The camera became unavailable. Close this sheet and try again, or enter its address.")
        }
    }

    // Start after presentation, and release the camera whenever the sheet leaves.
    final class ScannerController: UIViewController {
        let scanner = DataScannerViewController(recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .balanced, recognizesMultipleItems: false, isHighFrameRateTrackingEnabled: false,
            isPinchToZoomEnabled: true, isGuidanceEnabled: true, isHighlightingEnabled: true)
        var issue: ((String) -> Void)?

        override func viewDidLoad() {
            super.viewDidLoad()
            addChild(scanner)
            scanner.view.frame = view.bounds
            scanner.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            view.addSubview(scanner.view)
            scanner.didMove(toParent: self)
        }

        override func viewDidAppear(_ animated: Bool) {
            super.viewDidAppear(animated)
            do { try scanner.startScanning() }
            catch { issue?("The camera could not start. Close this sheet and try again, or enter its address.") }
        }

        override func viewWillDisappear(_ animated: Bool) {
            scanner.stopScanning()
            super.viewWillDisappear(animated)
        }
    }
}
