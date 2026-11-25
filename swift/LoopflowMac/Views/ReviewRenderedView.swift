#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// How a changed file reads in the Diff before its lines: Markdown as a
/// redline, a picture or page old beside new. Everything but a raster image
/// also has its line Diff, one click away as Source.
enum ReviewRendering: Equatable {
    case markdown, svg, html, image

    static func of(_ file: ReviewFile, awaitingPatch: Bool = false) -> ReviewRendering? {
        let ext = (file.path as NSString).pathExtension.lowercased()
        if (file.status == .binary || awaitingPatch), ["png", "jpg", "jpeg", "gif", "webp"].contains(ext) { return .image }
        if file.status == .binary { return nil }
        switch ext {
        case "md", "markdown": return .markdown
        case "svg": return .svg
        case "html", "htm": return .html
        default: return nil
        }
    }

    var hasSource: Bool { self != .image }
}

/// A rendered file in the stream, once both its sides are read; the stream
/// shows `ReviewSidesFailed` instead when they don't. Markdown
/// follows the layout picker: a redline in New code and Unified, two columns
/// side by side. A picture or page has no unified form, so every layout shows
/// both sides, stacked old above new in a pane too narrow for both.
struct ReviewRendered: View {
    let store: TaskFilesStore
    let reader: DiffReviewState.ReviewReader
    let file: ReviewFile
    let rendering: ReviewRendering
    let split: Bool
    @Environment(\.palette) private var palette

    private var reading: ComparisonReading { store.diffReview.reading(for: reader) }
    private var find: FileFind? { reader == .changes ? nil : store.find }

    var body: some View {
        if let sides = reading.sides(of: file) {
            switch rendering {
            case .markdown:
                ReviewMarkdown(old: sides.old?.content, new: sides.new?.content, split: split, find: find).equatable()
                    .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
            case .image, .svg, .html:
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: Spacing.md) { both(sides) }.frame(minWidth: DiffReviewState.splitWidth)
                    VStack(alignment: .leading, spacing: Spacing.md) { both(sides) }
                }
                .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
            }
        } else {
            ReviewNotice(text: "Reading…")
        }
    }

    /// Old then new, as pictures or as pages.
    @ViewBuilder private func both(_ sides: ComparisonReading.FileSides) -> some View {
        if rendering == .html {
            let count = [sides.old?.content, sides.new?.content].compactMap { $0 }.count
            page(sides.old, index: 0, count: count, absent: "Added")
                .task(id: find?.query) {
                    if count == 0, let find, !find.query.isEmpty { await find.search([]) }
                }
            // Relative references resolve in the working tree only when the new
            // side is the working tree of a checkout on this Machine.
            page(sides.new, index: 1, count: count, absent: "Deleted", baseURL: reading.comparison.head == nil && store.machine == nil
                 ? URL(fileURLWithPath: store.cwd).appendingPathComponent(file.path) : nil)
        } else {
            // No ideal width: a picture's natural size must not decide whether the
            // two sides fit beside each other; only the pane's width does.
            TaskImage(file: sides.old, absent: "Added").frame(minWidth: 0, idealWidth: 0, maxWidth: .infinity)
            TaskImage(file: sides.new, absent: "Deleted").frame(minWidth: 0, idealWidth: 0, maxWidth: .infinity)
        }
    }
    @ViewBuilder private func page(_ side: TaskFileSnapshot?, index: Int, count: Int, absent: String, baseURL: URL? = nil) -> some View {
        if let html = side?.content {
            TaskHTMLView(html: html, baseURL: baseURL, find: find, pageIndex: index, pageCount: count)
                .id("html-side-\(index)")
                .containerRelativeFrame(.vertical)
                .frame(maxWidth: .infinity)
                .overlay { RoundedRectangle(cornerRadius: CornerRadius.control).stroke(palette.border) }
                .accessibilityIdentifier("changes-rendered-html")
        } else {
            Text(absent).font(Typography.meta).foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity)
        }
    }
}

/// A file whose sides did not read: why, and a way to read them again.
struct ReviewSidesFailed: View {
    let store: TaskFilesStore
    let reader: DiffReviewState.ReviewReader
    let file: ReviewFile
    let error: String

    var body: some View {
        HStack(spacing: 0) {
            ReviewNotice(text: "Can't render this file: \(error)")
            Button("Retry") { Task { await store.diffReview.reading(for: reader).retrySides(file) } }
                .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
        }
    }
}

/// A changed Markdown document: one redline, removed words struck and added
/// words filled, or old and new in two columns, block beside block.
struct ReviewMarkdown: View, Equatable {
    let old: String?
    let new: String?
    let split: Bool
    var find: FileFind?
    @Environment(\.palette) private var palette

    nonisolated static func == (lhs: ReviewMarkdown, rhs: ReviewMarkdown) -> Bool {
        lhs.old == rhs.old && lhs.new == rhs.new && lhs.split == rhs.split && lhs.find === rhs.find
    }

    var body: some View {
        let redline = MarkdownRedline(old: old ?? "", new: new ?? "")
        let rows = redline.entries.map { entry in
            split ? [column(entry, showing: .removed), column(entry, showing: .added)] : [column(entry)]
        }
        Rows(rows: rows, split: split, find: find)
    }

    private struct Rows: View {
        let rows: [[Column?]]
        let split: Bool
        let find: FileFind?
        @Environment(\.palette) private var palette

        var body: some View {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(rows.indices, id: \.self) { index in
                    if split {
                        HStack(alignment: .top, spacing: Spacing.md) {
                            block(rows[index][0], index: index * 2).frame(maxWidth: .infinity, alignment: .leading)
                            Divider()
                            block(rows[index][1], index: index * 2 + 1).frame(maxWidth: .infinity, alignment: .leading)
                        }
                    } else {
                        block(rows[index][0], index: index * 2).frame(maxWidth: 720, alignment: .leading)
                    }
                }
            }
            .task(id: FileFind.Scan(query: find?.query, revision: rows)) {
                if let find, !find.query.isEmpty {
                    await find.search(rows.enumerated().flatMap { index, row in
                        row.enumerated().flatMap { side, column in column?.block.searchCells(index: index * 2 + side) ?? [] }
                    })
                }
            }
            .textSelection(.enabled)
            .tint(palette.linkInk)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier(split ? "changes-markdown-columns" : "changes-markdown-redline")
        }

        @ViewBuilder private func block(_ column: Column?, index: Int) -> some View {
            if let column {
                let row = MarkdownBlocks.Row(block: column.block.highlighted(find, index: index, color: palette.keyboardFocus.opacity(0.45)))
                Group {
                    if let change = column.change {
                        let tone = change == .removed ? palette.failureTone : palette.successTone
                        row.padding(.horizontal, Spacing.xs).padding(.vertical, Spacing.xxs)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(tone.fill)
                            .overlay(alignment: .leading) { Rectangle().fill(tone.ink).frame(width: 2) }
                    } else { row }
                }.id(MarkdownBlocks.anchor(index))
            } else { Color.clear.frame(height: 1) }
        }
    }

    private struct Column: Equatable {
        let block: MarkdownBlocks.Block
        var change: ReviewWords.Kind? = nil
    }

    private func column(_ entry: MarkdownRedline.Entry, showing side: ReviewWords.Kind? = nil) -> Column? {
        switch entry {
        case .same(let block): return Column(block: block)
        case .removed(let block): return side == .added ? nil : Column(block: struck(block), change: .removed)
        case .added(let block): return side == .removed ? nil : Column(block: block, change: .added)
        case .changed(let old, let new, let words):
            return Column(block: text(words, old: old, new: new, showing: side.map { [.same, $0] } ?? [.same, .removed, .added]))
        }
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
        VStack(spacing: Spacing.xs) {
            if let file, file.state != .missing {
                if let data = Self.bytes(of: file), let image = NSImage(data: data) {
                    Image(nsImage: image).resizable().interpolation(.high).aspectRatio(contentMode: .fit)
                        // Its height follows its width, so the backing hugs the picture.
                        .frame(maxWidth: image.size.width).fixedSize(horizontal: false, vertical: true)
                        .background(palette.surfaceMuted)
                        .accessibilityLabel("\((file.path as NSString).lastPathComponent), \(Self.size(of: image))")
                    caption("\(Self.size(of: image)) · \(Self.byteCount(file.sizeBytes))")
                } else if file.state == .truncated || (file.state == .binary && file.data == nil) {
                    caption("Image too large to show · \(Self.byteCount(file.sizeBytes))")
                } else {
                    caption("Can't decode this image")
                }
            } else {
                caption(absent)
            }
        }
    }

    /// A binary file's bytes; an SVG is text, read as its own bytes.
    private static func bytes(of file: TaskFileSnapshot) -> Data? {
        if let data = file.data { return data }
        guard file.state == .text, (file.path as NSString).pathExtension.lowercased() == "svg" else { return nil }
        return file.content.map { Data($0.utf8) }
    }

    private func caption(_ text: String) -> some View {
        Text(text).font(Typography.meta).foregroundStyle(palette.textSecondary)
    }

    /// The image's pixel size, or its point size for a picture without pixels.
    private static func size(of image: NSImage) -> String {
        let pixels = image.representations.first.map { CGSize(width: $0.pixelsWide, height: $0.pixelsHigh) }
        let size = pixels.flatMap { $0.width > 0 && $0.height > 0 ? $0 : nil } ?? image.size
        return "\(Int(size.width))×\(Int(size.height))"
    }

    private static func byteCount(_ count: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: count), countStyle: .file)
    }
}
#endif
