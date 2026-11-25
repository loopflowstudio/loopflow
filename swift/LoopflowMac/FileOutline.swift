#if os(macOS)
import Foundation
import Loopflow
import SwiftTreeSitter

struct FileOutline: Sendable {
    struct Entry: Sendable {
        let symbol: TaskSymbol
        let lastLine: Int
    }

    let entries: [Entry]

    func enclosing(_ line: Int) -> TaskSymbol? {
        entries.last { $0.symbol.line <= line && line <= $0.lastLine }?.symbol
    }
}

extension FileOutline {
    init(tree: MutableTree?, text: String, path: String) {
        guard let root = tree?.rootNode else { self.entries = []; return }
        let source = text as NSString
        var entries: [FileOutline.Entry] = []
        var stack: [(Node, String?)] = [(root, nil)]
        while let (node, parent) = stack.popLast() {
            var container = parent
            if let (kind, label) = Self.declaration(node) {
                let name = source.substring(with: label.range).trimmingCharacters(in: .whitespacesAndNewlines)
                let last = node.pointRange.upperBound
                let symbol = TaskSymbol(name: name, kind: kind, path: path, line: Int(label.pointRange.lowerBound.row) + 1,
                                        container: parent)
                entries.append(.init(symbol: symbol, lastLine: max(symbol.line, Int(last.row) + (last.column > 0 ? 1 : 0))))
                container = name
            }
            for i in (0..<node.namedChildCount).reversed() {
                if let child = node.namedChild(at: i) { stack.append((child, container)) }
            }
        }
        self.entries = entries
    }

    private static func declaration(_ node: Node) -> (String, Node)? {
        guard let type = node.nodeType else { return nil }
        if type == "section", let heading = node.firstChild,
           ["atx_heading", "setext_heading"].contains(heading.nodeType),
           let title = (0..<heading.namedChildCount).compactMap({ heading.namedChild(at: $0) })
                .first(where: { $0.nodeType == "inline" || $0.nodeType == "paragraph" }) {
            return ("heading", title)
        }
        let kind: String
        switch type {
        case "function_declaration", "function_definition", "function_item", "function_signature_item",
             "function_signature", "generator_function_declaration", "protocol_function_declaration":
            kind = "function"
        case "method_definition", "method_declaration", "method_signature", "method", "singleton_method":
            kind = "method"
        case "impl_item": kind = "impl"
        case "class_declaration" where node.child(byFieldName: "declaration_kind")?.nodeType == "extension": kind = "extension"
        case "class_definition", "class_declaration", "class_specifier", "class", "object_declaration",
             "struct_item", "struct_specifier", "struct_declaration", "enum_item", "enum_declaration",
             "type_spec", "type_item", "type_alias_declaration", "interface_declaration", "protocol_declaration",
             "trait_item", "mod_item", "module":
            kind = "type"
        case "variable_declarator" where ["arrow_function", "function_expression"].contains(node.child(byFieldName: "value")?.nodeType):
            kind = "function"
        default: return nil
        }
        if var label = node.child(byFieldName: "name") ?? node.child(byFieldName: "type") ?? node.child(byFieldName: "declarator") {
            while let inner = label.child(byFieldName: "declarator") { label = inner }
            return (kind, label)
        }
        for i in 0..<node.namedChildCount {
            if let label = node.namedChild(at: i), label.nodeType?.hasSuffix("identifier") == true { return (kind, label) }
        }
        return nil
    }
}
#endif
