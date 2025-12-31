import SwiftUI
import Foundation

@MainActor
class BrowserViewModel: ObservableObject {
    @Published var urlInput: String = ""
    @Published var currentURL: String = ""
    @Published var document: DOMNode?
    @Published var error: String?
    @Published var isLoading: Bool = false
    @Published var canGoBack: Bool = false
    @Published var canGoForward: Bool = false

    var securityIcon: String {
        if currentURL.hasPrefix("file://") || !currentURL.contains("://") {
            return "doc"
        } else if currentURL.hasPrefix("https://") {
            return "lock.fill"
        } else {
            return "globe"
        }
    }

    var securityColor: Color {
        if currentURL.hasPrefix("file://") || !currentURL.contains("://") {
            return .secondary
        } else if currentURL.hasPrefix("https://") {
            return .green
        } else {
            return .secondary
        }
    }

    func loadURL() {
        let url = urlInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty else { return }

        isLoading = true
        error = nil
        document = nil
        currentURL = url

        // Resolve path
        let path: String
        if url.hasPrefix("file://") {
            path = String(url.dropFirst(7))
        } else {
            path = url
        }

        // Try to load the file
        do {
            let html = try String(contentsOfFile: path, encoding: .utf8)
            // For now, use a simple Swift HTML parser
            // Later we'll call into Rust
            document = parseHTML(html)
            print("[INFO] Successfully loaded: \(path)")
        } catch {
            self.error = "Failed to load: \(error.localizedDescription)"
            print("[ERROR] Failed to load \(path): \(error)")
        }

        isLoading = false
    }

    func goBack() {
        // TODO: Implement history
    }

    func goForward() {
        // TODO: Implement history
    }

    func refresh() {
        loadURL()
    }

    // MARK: - Simple HTML Parser (temporary - will be replaced by Rust)

    private func parseHTML(_ html: String) -> DOMNode {
        let parser = SimpleHTMLParser(html: html)
        return parser.parse()
    }
}

// MARK: - DOM Node

enum DOMNodeType: Equatable {
    case document
    case element(tagName: String, attributes: [String: String])
    case text(String)
    case comment(String)
}

class DOMNode: Identifiable, ObservableObject {
    let id = UUID()
    var type: DOMNodeType
    var children: [DOMNode]

    init(type: DOMNodeType, children: [DOMNode] = []) {
        self.type = type
        self.children = children
    }
}

// MARK: - Simple HTML Parser (temporary)

class SimpleHTMLParser {
    private let html: String
    private var index: String.Index

    init(html: String) {
        self.html = html
        self.index = html.startIndex
    }

    func parse() -> DOMNode {
        let document = DOMNode(type: .document)
        document.children = parseNodes()
        return document
    }

    private func parseNodes() -> [DOMNode] {
        var nodes: [DOMNode] = []

        while index < html.endIndex {
            if peek() == "<" {
                if peekAhead(2) == "<!" {
                    // Skip DOCTYPE or comment
                    skipUntil(">")
                    advance()
                } else if peekAhead(2) == "</" {
                    // End tag - return to parent
                    break
                } else {
                    // Start tag
                    if let element = parseElement() {
                        nodes.append(element)
                    }
                }
            } else {
                // Text content
                let text = parseText()
                if !text.isEmpty {
                    nodes.append(DOMNode(type: .text(text)))
                }
            }
        }

        return nodes
    }

    private func parseElement() -> DOMNode? {
        guard consume("<") else { return nil }

        // Parse tag name
        let tagName = parseTagName().lowercased()
        guard !tagName.isEmpty else { return nil }

        // Parse attributes
        let attributes = parseAttributes()

        // Skip to end of tag
        let isSelfClosing = skipWhitespace() && peek() == "/"
        if isSelfClosing { advance() }
        guard consume(">") else {
            skipUntil(">")
            advance()
            return nil
        }

        let node = DOMNode(type: .element(tagName: tagName, attributes: attributes))

        // Void elements
        let voidElements = ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"]
        if isSelfClosing || voidElements.contains(tagName) {
            return node
        }

        // Parse children
        node.children = parseNodes()

        // Parse end tag
        if peek() == "<" && peekAhead(2) == "</" {
            skipUntil(">")
            advance()
        }

        return node
    }

    private func parseTagName() -> String {
        var name = ""
        while index < html.endIndex {
            let c = html[index]
            if c.isLetter || c.isNumber || c == "-" || c == "_" {
                name.append(c)
                advance()
            } else {
                break
            }
        }
        return name
    }

    private func parseAttributes() -> [String: String] {
        var attrs: [String: String] = [:]

        while index < html.endIndex {
            skipWhitespace()
            let c = peek()
            if c == ">" || c == "/" { break }

            let name = parseAttributeName()
            if name.isEmpty { break }

            var value = ""
            skipWhitespace()
            if peek() == "=" {
                advance()
                skipWhitespace()
                value = parseAttributeValue()
            }

            attrs[name] = value
        }

        return attrs
    }

    private func parseAttributeName() -> String {
        var name = ""
        while index < html.endIndex {
            let c = html[index]
            if c.isLetter || c.isNumber || c == "-" || c == "_" || c == ":" {
                name.append(c)
                advance()
            } else {
                break
            }
        }
        return name
    }

    private func parseAttributeValue() -> String {
        let quote = peek()
        if quote == "\"" || quote == "'" {
            advance()
            var value = ""
            while index < html.endIndex && html[index] != quote {
                value.append(html[index])
                advance()
            }
            if index < html.endIndex { advance() }
            return value
        } else {
            // Unquoted value
            var value = ""
            while index < html.endIndex {
                let c = html[index]
                if c.isWhitespace || c == ">" || c == "/" {
                    break
                }
                value.append(c)
                advance()
            }
            return value
        }
    }

    private func parseText() -> String {
        var text = ""
        while index < html.endIndex && html[index] != "<" {
            text.append(html[index])
            advance()
        }
        return text
    }

    // MARK: - Helper methods

    private func peek() -> Character {
        guard index < html.endIndex else { return "\0" }
        return html[index]
    }

    private func peekAhead(_ count: Int) -> String {
        let end = html.index(index, offsetBy: count, limitedBy: html.endIndex) ?? html.endIndex
        return String(html[index..<end])
    }

    private func advance() {
        if index < html.endIndex {
            index = html.index(after: index)
        }
    }

    private func consume(_ expected: Character) -> Bool {
        if peek() == expected {
            advance()
            return true
        }
        return false
    }

    private func consume(_ expected: String) -> Bool {
        for c in expected {
            if !consume(c) { return false }
        }
        return true
    }

    @discardableResult
    private func skipWhitespace() -> Bool {
        var skipped = false
        while index < html.endIndex && html[index].isWhitespace {
            advance()
            skipped = true
        }
        return skipped
    }

    private func skipUntil(_ target: Character) {
        while index < html.endIndex && html[index] != target {
            advance()
        }
    }
}
