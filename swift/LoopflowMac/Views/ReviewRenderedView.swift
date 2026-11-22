#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// How a changed file reads in the Diff before its lines: Markdown as a
/// redline, a picture or page old beside new. Everything but a raster image
/// also has its line Diff, one click away as Source.
enum ReviewRendering: Equatable {
    case markdown, svg, html, image

    static func of(_ file: ReviewFile) -> ReviewRendering? {
        let ext = (file.path as NSString).pathExtension.lowercased()
        if file.status == .binary { return ["png", "jpg", "jpeg", "gif", "webp"].contains(ext) ? .image : nil }
        switch ext {
        case "md", "markdown": return .markdown
        case "svg": return .svg
        case "html", "htm": return .html
        default: return nil
        }
    }

    var hasSource: Bool { self != .image }
}

/// A rendered file in the stream, once both its sides are read. Markdown
/// follows the layout picker: a redline in New code and Unified, two columns
/// side by side. A picture or page has no unified form, so every layout shows
/// both sides, stacked old above new in a pane too narrow for both.
struct ReviewRendered: View {
    let store: TaskFilesStore
    let file: ReviewFile
    let rendering: ReviewRendering
    let split: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        if let sides = store.sides(of: file) {
            switch rendering {
            case .markdown:
                ReviewMarkdown(old: sides.old?.content, new: sides.new?.content, split: split).equatable()
                    .padding(.horizontal, 12).padding(.vertical, 10)
            case .image, .svg, .html:
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 12) { both(sides) }.frame(minWidth: TaskFilesStore.splitWidth)
                    VStack(alignment: .leading, spacing: 12) { both(sides) }
                }
                .padding(.horizontal, 12).padding(.vertical, 10)
            }
        } else if let error = store.sideErrors[file.path] {
            ReviewSidesFailed(store: store, file: file, error: error)
        } else {
            ReviewNotice(text: "Reading…")
        }
    }

    /// Old then new, as pictures or as pages.
    @ViewBuilder private func both(_ sides: TaskFilesStore.FileSides) -> some View {
        if rendering == .html {
            page(sides.old, absent: "Added")
            // Relative references resolve in the working tree only when the new
            // side is the working tree of a checkout on this Machine.
            page(sides.new, absent: "Deleted", baseURL: sides.commits.last == "" && store.query.remoteMachine == nil
                 ? URL(fileURLWithPath: store.cwd).appendingPathComponent(file.path) : nil)
        } else {
            TaskImage(file: sides.old, absent: "Added").frame(maxWidth: .infinity)
            TaskImage(file: sides.new, absent: "Deleted").frame(maxWidth: .infinity)
        }
    }
    @ViewBuilder private func page(_ side: TaskFileSnapshot?, absent: String, baseURL: URL? = nil) -> some View {
        if let html = side?.content {
            TaskHTMLView(html: html, baseURL: baseURL, interacted: store.noteInteraction)
                .containerRelativeFrame(.vertical)
                .frame(maxWidth: .infinity)
                .overlay { RoundedRectangle(cornerRadius: CornerRadius.sm).stroke(palette.border) }
                .accessibilityIdentifier("changes-rendered-html")
        } else {
            Text(absent).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity)
        }
    }
}

/// A file whose sides did not read: why, and a way to read them again.
struct ReviewSidesFailed: View {
    let store: TaskFilesStore
    let file: ReviewFile
    let error: String

    var body: some View {
        HStack(spacing: 0) {
            ReviewNotice(text: "Can't render this file: \(error)")
            Button("Retry") { Task { await store.retrySides(file) } }
                .buttonStyle(WorkPlainButtonStyle()).font(Typography.caption(11))
        }
    }
}

/// A changed Markdown document: one redline, removed words struck and added
/// words filled, or old and new in two columns, block beside block.
struct ReviewMarkdown: View, Equatable {
    let old: String?
    let new: String?
    let split: Bool
    @Environment(\.palette) private var palette

    nonisolated static func == (lhs: ReviewMarkdown, rhs: ReviewMarkdown) -> Bool {
        lhs.old == rhs.old && lhs.new == rhs.new && lhs.split == rhs.split
    }

    var body: some View {
        let redline = MarkdownRedline(old: old ?? "", new: new ?? "")
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ForEach(Array(redline.entries.enumerated()), id: \.offset) { _, entry in
                if split {
                    HStack(alignment: .top, spacing: 12) {
                        side(entry, showing: .removed).frame(maxWidth: .infinity, alignment: .leading)
                        Divider()
                        side(entry, showing: .added).frame(maxWidth: .infinity, alignment: .leading)
                    }
                } else {
                    redlined(entry).frame(maxWidth: 720, alignment: .leading)
                }
            }
        }
        .textSelection(.enabled)
        .tint(palette.linkInk)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier(split ? "changes-markdown-columns" : "changes-markdown-redline")
    }

    @ViewBuilder private func redlined(_ entry: MarkdownRedline.Entry) -> some View {
        switch entry {
        case .same(let block): MarkdownBlocks.Row(block: block)
        case .removed(let block): marked(struck(block), tone: palette.failureTone)
        case .added(let block): marked(block, tone: palette.successTone)
        case .changed(let old, let new, let words):
            MarkdownBlocks.Row(block: text(words, old: old, new: new, showing: [.same, .removed, .added]))
        }
    }

    /// One column of an entry: the old blocks on the left, the new on the right.
    @ViewBuilder private func side(_ entry: MarkdownRedline.Entry, showing kind: ReviewWords.Kind) -> some View {
        switch entry {
        case .same(let block): MarkdownBlocks.Row(block: block)
        case .removed(let block):
            if kind == .removed { marked(struck(block), tone: palette.failureTone) } else { Color.clear.frame(height: 1) }
        case .added(let block):
            if kind == .added { marked(block, tone: palette.successTone) } else { Color.clear.frame(height: 1) }
        case .changed(let old, let new, let words):
            MarkdownBlocks.Row(block: text(words, old: old, new: new, showing: [.same, kind]))
        }
    }

    /// A block that stands alone, removed or added: filled in its tone with a bar beside it.
    private func marked(_ block: MarkdownBlocks.Block, tone: ThemeTone) -> some View {
        MarkdownBlocks.Row(block: block)
            .padding(.horizontal, Spacing.xs).padding(.vertical, 2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(tone.fill)
            .overlay(alignment: .leading) { Rectangle().fill(tone.ink).frame(width: 2) }
    }

    private func struck(_ block: MarkdownBlocks.Block) -> MarkdownBlocks.Block {
        var block = block
        block.cells = block.cells.map { cell in
            var cell = cell
            cell.strikethroughStyle = .single
            cell.foregroundColor = palette.failureTone.ink
            return cell
        }
        return block
    }

    /// The changed block with the edit's words: removed ones struck in the
    /// removed tone, added ones filled in the added tone. It keeps the new
    /// block's kind and marker unless it shows the old side alone.
    private func text(_ words: [ReviewWords], old: MarkdownBlocks.Block, new: MarkdownBlocks.Block,
                      showing kinds: Set<ReviewWords.Kind>) -> MarkdownBlocks.Block {
        var text = AttributedString()
        for (piece, kind) in MarkdownRedline.pieces(words, old: old.text, new: new.text, showing: kinds) {
            var piece = piece
            switch kind {
            case .same: break
            case .removed:
                piece.strikethroughStyle = .single
                piece.foregroundColor = palette.failureTone.ink
                piece.backgroundColor = palette.failureTone.ink.opacity(0.18)
            case .added:
                piece.backgroundColor = palette.successTone.ink.opacity(0.26)
            }
            text += piece
        }
        var block = kinds.contains(.added) ? new : old
        block.cells = [text]
        return block
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
                if let data = Self.bytes(of: file), let image = NSImage(data: data) {
                    Image(nsImage: image).resizable().interpolation(.high).aspectRatio(contentMode: .fit)
                        .frame(maxWidth: image.size.width, maxHeight: image.size.height)
                        .background(palette.surfaceMuted)
                        .accessibilityLabel("\((file.path as NSString).lastPathComponent), \(Self.size(of: image))")
                    caption("\(Self.size(of: image)) · \(Self.bytes(file.sizeBytes))")
                } else if file.state == .truncated || (file.state == .binary && file.data == nil) {
                    caption("Image too large to show · \(Self.bytes(file.sizeBytes))")
                } else {
                    caption("Can't decode this image")
                }
            } else {
                caption(absent)
            }
        }
    }

    /// A binary file's bytes; an SVG is text, read as its own bytes.
    static func bytes(of file: TaskFileSnapshot) -> Data? {
        if let data = file.data { return data }
        guard file.state == .text, (file.path as NSString).pathExtension.lowercased() == "svg" else { return nil }
        return file.content.map { Data($0.utf8) }
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
