#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// A changed image in the Diff: old left and new right, each with its pixel
/// and byte size, stacked old above new in a pane too narrow for both. An
/// image has no unified form, so every layout shows both sides.
struct ReviewImages: View {
    let store: TaskFilesStore
    let file: ReviewFile
    @Environment(\.palette) private var palette

    /// Binary files the Diff shows as pictures. Files' Read shows any binary
    /// that decodes as one.
    static func shows(_ file: ReviewFile) -> Bool {
        file.status == .binary && ["png", "jpg", "jpeg", "gif", "webp"]
            .contains((file.path as NSString).pathExtension.lowercased())
    }

    var body: some View {
        if let sides = store.sides(of: file) {
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: 12) { panes(sides) }.frame(minWidth: 640)
                VStack(alignment: .leading, spacing: 12) { panes(sides) }
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
        } else if let error = store.sideErrors[file.path] {
            HStack(spacing: 0) {
                ReviewNotice(text: "Image unavailable: \(error)")
                Button("Retry") { Task { await store.retrySides(file) } }
                    .buttonStyle(WorkPlainButtonStyle()).font(Typography.caption(11))
            }
        } else {
            ReviewNotice(text: "Reading…")
        }
    }

    @ViewBuilder private func panes(_ sides: TaskFilesStore.FileSides) -> some View {
        TaskImage(file: sides.old, absent: "Added").frame(maxWidth: .infinity)
        TaskImage(file: sides.new, absent: "Deleted").frame(maxWidth: .infinity)
    }
}

/// One side of a file read as a picture, with `W×H · size` under it; what
/// stands in when there is no picture to show.
struct TaskImage: View {
    let file: TaskFileSnapshot?
    /// Shown when this side has no file.
    var absent = "File no longer exists"
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 6) {
            if let file, file.state != .missing {
                if let data = file.data, let image = NSImage(data: data) {
                    Image(nsImage: image).resizable().interpolation(.high).aspectRatio(contentMode: .fit)
                        .frame(maxWidth: image.size.width, maxHeight: image.size.height)
                        .background(palette.surfaceMuted)
                        .accessibilityLabel("\((file.path as NSString).lastPathComponent), \(Self.size(of: image))")
                    caption("\(Self.size(of: image)) · \(Self.bytes(file.sizeBytes))")
                } else if file.state == .binary, file.data == nil {
                    caption("Image too large to show · \(Self.bytes(file.sizeBytes))")
                } else {
                    caption("Can't decode this image")
                }
            } else {
                caption(absent)
            }
        }
    }

    private func caption(_ text: String) -> some View {
        Text(text).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
    }

    /// The image's pixel size, or its point size for a picture without pixels.
    static func size(of image: NSImage) -> String {
        let pixels = image.representations.first.map { CGSize(width: $0.pixelsWide, height: $0.pixelsHigh) }
        let size = pixels.flatMap { $0.width > 0 && $0.height > 0 ? $0 : nil } ?? image.size
        return "\(Int(size.width))×\(Int(size.height))"
    }

    static func bytes(_ count: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: count), countStyle: .file)
    }
}
#endif
