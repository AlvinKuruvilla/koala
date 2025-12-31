import SwiftUI
import KoalaCore

struct BrowserView: View {
    @StateObject private var viewModel = BrowserViewModel()

    var body: some View {
        VStack(spacing: 0) {
            // Toolbar
            ToolbarView(viewModel: viewModel)

            Divider()

            // Content area
            ContentView(viewModel: viewModel)
        }
        .frame(minWidth: 800, minHeight: 600)
    }
}

// MARK: - Toolbar

struct ToolbarView: View {
    @ObservedObject var viewModel: BrowserViewModel
    @FocusState private var isUrlFieldFocused: Bool

    var body: some View {
        HStack(spacing: 12) {
            // Navigation buttons
            HStack(spacing: 4) {
                Button(action: { viewModel.goBack() }) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 14, weight: .medium))
                }
                .buttonStyle(NavButtonStyle())
                .disabled(!viewModel.canGoBack)

                Button(action: { viewModel.goForward() }) {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 14, weight: .medium))
                }
                .buttonStyle(NavButtonStyle())
                .disabled(!viewModel.canGoForward)

                Button(action: { viewModel.refresh() }) {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 14, weight: .medium))
                }
                .buttonStyle(NavButtonStyle())
                .disabled(viewModel.currentURL.isEmpty)
            }

            // URL Bar
            HStack(spacing: 8) {
                // Security indicator
                Image(systemName: viewModel.securityIcon)
                    .font(.system(size: 12))
                    .foregroundColor(viewModel.securityColor)

                TextField("Enter file path or URL...", text: $viewModel.urlInput)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .focused($isUrlFieldFocused)
                    .onSubmit {
                        viewModel.loadURL()
                    }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color(nsColor: .controlBackgroundColor))
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isUrlFieldFocused ? Color.accentColor : Color.gray.opacity(0.3), lineWidth: 1)
            )

            // Debug panel toggle
            Button(action: { viewModel.showDebugPanel.toggle() }) {
                Image(systemName: viewModel.showDebugPanel ? "ladybug.fill" : "ladybug")
                    .font(.system(size: 14, weight: .medium))
            }
            .buttonStyle(NavButtonStyle())
            .help("Toggle Debug Panel")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

// MARK: - Content View

struct ContentView: View {
    @ObservedObject var viewModel: BrowserViewModel

    var body: some View {
        HSplitView {
            // Main content
            mainContent
                .frame(minWidth: 300)

            // Debug panel (shown when toggle is on)
            if viewModel.showDebugPanel {
                DebugPanelView(viewModel: viewModel)
                    .frame(minWidth: 300, idealWidth: 400)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    var mainContent: some View {
        Group {
            if let error = viewModel.error {
                ErrorView(message: error)
            } else if viewModel.isLoading {
                ProgressView("Loading...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let document = viewModel.document {
                ScrollView {
                    DocumentView(node: document)
                        .padding()
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                WelcomeView()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: .textBackgroundColor))
    }
}

// MARK: - Welcome View

struct WelcomeView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "globe")
                .font(.system(size: 64))
                .foregroundColor(.secondary)

            Text("Welcome to Koala")
                .font(.title)
                .fontWeight(.semibold)

            Text("Enter an absolute file path to load HTML")
                .font(.subheadline)
                .foregroundColor(.secondary)

            Text("Try: /Users/alvinkuruvilla/Dev/koala/res/simple.html")
                .font(.caption)
                .foregroundColor(.gray)
                .padding(.top, 4)
                .textSelection(.enabled)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Error View

struct ErrorView: View {
    let message: String

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 40))
                .foregroundColor(.orange)

            Text("Unable to load page")
                .font(.headline)

            Text(message)
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Debug Panel

struct DebugPanelView: View {
    @ObservedObject var viewModel: BrowserViewModel
    @State private var selectedTab = 0

    var body: some View {
        VStack(spacing: 0) {
            // Tab bar
            HStack(spacing: 0) {
                DebugTabButton(title: "DOM Tree", isSelected: selectedTab == 0) {
                    selectedTab = 0
                }
                DebugTabButton(title: "Raw HTML", isSelected: selectedTab == 1) {
                    selectedTab = 1
                }
                DebugTabButton(title: "Raw JSON", isSelected: selectedTab == 2) {
                    selectedTab = 2
                }
            }
            .padding(.horizontal, 8)
            .padding(.top, 8)

            Divider()
                .padding(.top, 8)

            // Tab content
            Group {
                switch selectedTab {
                case 0:
                    if let document = viewModel.document {
                        ScrollView {
                            DOMTreeView(node: document, depth: 0)
                                .padding()
                        }
                    } else {
                        Text("No document loaded")
                            .foregroundColor(.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                case 1:
                    ScrollView {
                        Text(viewModel.rawHTML)
                            .font(.system(size: 11, design: .monospaced))
                            .textSelection(.enabled)
                            .padding()
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                case 2:
                    ScrollView {
                        Text(formatJSON(viewModel.rawJSON))
                            .font(.system(size: 11, design: .monospaced))
                            .textSelection(.enabled)
                            .padding()
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                default:
                    EmptyView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(Color(nsColor: .controlBackgroundColor))
    }

    func formatJSON(_ json: String) -> String {
        // Try to pretty-print the JSON
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data),
              let prettyData = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]),
              let pretty = String(data: prettyData, encoding: .utf8) else {
            return json
        }
        return pretty
    }
}

struct DebugTabButton: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 11, weight: isSelected ? .semibold : .regular))
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(isSelected ? Color.accentColor.opacity(0.2) : Color.clear)
                .cornerRadius(4)
        }
        .buttonStyle(.plain)
    }
}

struct DOMTreeView: View {
    let node: DOMNode
    let depth: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // Node representation
            HStack(spacing: 4) {
                Text(String(repeating: "  ", count: depth))
                    .font(.system(size: 11, design: .monospaced))

                nodeLabel
            }

            // Children
            ForEach(node.childNodes) { child in
                DOMTreeView(node: child, depth: depth + 1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    var nodeLabel: some View {
        switch node.type {
        case "document":
            Text("#document")
                .font(.system(size: 11, design: .monospaced))
                .foregroundColor(.secondary)

        case "element":
            if let tagName = node.tagName {
                HStack(spacing: 2) {
                    Text("<\(tagName)>")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.blue)

                    // Show computed styles summary
                    if let style = node.computedStyle {
                        stylesSummary(style)
                    }
                }
            }

        case "text":
            if let content = node.content {
                let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    let preview = trimmed.count > 40 ? String(trimmed.prefix(40)) + "..." : trimmed
                    Text("\"\(preview)\"")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.green)
                }
            }

        case "comment":
            if let content = node.content {
                let preview = content.count > 30 ? String(content.prefix(30)) + "..." : content
                Text("<!-- \(preview) -->")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(.gray)
            }

        default:
            Text(node.type)
                .font(.system(size: 11, design: .monospaced))
                .foregroundColor(.secondary)
        }
    }

    func stylesSummary(_ style: ComputedStyle) -> some View {
        let parts = buildStyleParts(style)
        return Group {
            if !parts.isEmpty {
                Text("[\(parts.joined(separator: " "))]")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.orange)
            }
        }
    }

    func buildStyleParts(_ style: ComputedStyle) -> [String] {
        var parts: [String] = []

        if let color = style.color {
            parts.append("color:#\(colorHex(color))")
        }
        if let bg = style.background_color {
            parts.append("bg:#\(colorHex(bg))")
        }
        if let fs = style.font_size {
            parts.append("font:\(Int(fs.cgFloat))px")
        }
        if let p = style.padding_top {
            parts.append("pad:\(Int(p.cgFloat))px")
        }
        if let m = style.margin_top {
            parts.append("margin:\(Int(m.cgFloat))px")
        }
        if style.border_top != nil {
            parts.append("border")
        }
        if let lh = style.line_height {
            parts.append("lh:\(String(format: "%.1f", lh))")
        }

        return parts
    }

    func colorHex(_ color: CSSColor) -> String {
        String(format: "%02x%02x%02x", color.r, color.g, color.b)
    }
}

// MARK: - Document Rendering

struct DocumentView: View {
    let node: DOMNode

    var body: some View {
        NodeView(node: node)
    }
}

struct NodeView: View {
    let node: DOMNode

    var body: some View {
        switch node.type {
        case "document":
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.childNodes) { child in
                    NodeView(node: child)
                }
            }

        case "element":
            if let tagName = node.tagName {
                ElementView(tagName: tagName, node: node)
            }

        case "text":
            if let content = node.content {
                let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    Text(normalizeWhitespace(content))
                }
            }

        case "comment":
            EmptyView()

        default:
            EmptyView()
        }
    }

    func normalizeWhitespace(_ text: String) -> String {
        text.components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}

struct ElementView: View {
    let tagName: String
    let node: DOMNode

    // Computed style shortcut
    var style: ComputedStyle? {
        node.computedStyle
    }

    var body: some View {
        switch tagName.lowercased() {
        case "html":
            // HTML element - just render children
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.childNodes) { child in
                    NodeView(node: child)
                }
            }

        case "body":
            // Body element - apply background and padding from CSS
            VStack(alignment: .leading, spacing: 8) {
                ForEach(node.childNodes) { child in
                    NodeView(node: child)
                }
            }
            .padding(style?.paddingInsets ?? EdgeInsets())
            .padding(style?.marginInsets ?? EdgeInsets())
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(style?.background_color?.swiftUIColor ?? Color.clear)
            .foregroundColor(style?.color?.swiftUIColor)

        case "div", "article", "section", "main", "header", "footer", "nav", "aside":
            // Block elements - apply box model and background
            styledBlockView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(node.childNodes) { child in
                        NodeView(node: child)
                    }
                }
            }

        case "head", "title", "meta", "link", "script", "style":
            EmptyView()

        case "h1":
            styledTextView(getTextContent(node), defaultSize: 32, weight: .bold)
                .padding(.vertical, 12)

        case "h2":
            styledTextView(getTextContent(node), defaultSize: 28, weight: .bold)
                .padding(.vertical, 10)

        case "h3":
            styledTextView(getTextContent(node), defaultSize: 24, weight: .semibold)
                .padding(.vertical, 8)

        case "p":
            styledTextView(getTextContent(node), defaultSize: 16, weight: .regular)
                .lineSpacing(style?.line_height.map { CGFloat(($0 - 1.0) * 16) } ?? 4)
                .padding(.bottom, style?.margin_bottom?.cgFloat ?? 8)

        case "span":
            // Inline element with possible highlight
            styledInlineView {
                Text(getTextContent(node))
            }

        case "a":
            Text(getTextContent(node))
                .foregroundColor(style?.color?.swiftUIColor ?? .blue)

        case "b", "strong":
            Text(getTextContent(node))
                .fontWeight(.bold)
                .foregroundColor(style?.color?.swiftUIColor)

        case "i", "em":
            Text(getTextContent(node))
                .italic()
                .foregroundColor(style?.color?.swiftUIColor)

        case "br":
            Text("\n")

        case "hr":
            Divider()
                .padding(.vertical, 16)

        default:
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.childNodes) { child in
                    NodeView(node: child)
                }
            }
        }
    }

    // MARK: - Styled Views

    /// Apply styles to a block-level element
    @ViewBuilder
    func styledBlockView<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        let baseView = content()
            .padding(style?.paddingInsets ?? EdgeInsets())
            .background(style?.background_color?.swiftUIColor ?? Color.clear)
            .foregroundColor(style?.color?.swiftUIColor)

        // Apply border if present
        if let border = style?.border_top {
            baseView
                .overlay(
                    RoundedRectangle(cornerRadius: 0)
                        .stroke(border.color.swiftUIColor, lineWidth: border.width.cgFloat)
                )
                .padding(style?.marginInsets ?? EdgeInsets())
        } else {
            baseView
                .padding(style?.marginInsets ?? EdgeInsets())
        }
    }

    /// Apply styles to inline element (like span with highlight)
    @ViewBuilder
    func styledInlineView<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        if let bgColor = style?.background_color {
            content()
                .foregroundColor(style?.color?.swiftUIColor)
                .padding(.horizontal, style?.padding_right?.cgFloat ?? 0)
                .padding(.vertical, style?.padding_top?.cgFloat ?? 0)
                .background(bgColor.swiftUIColor)
        } else {
            content()
                .foregroundColor(style?.color?.swiftUIColor)
        }
    }

    /// Create styled text with computed font size and color
    func styledTextView(_ text: String, defaultSize: CGFloat, weight: Font.Weight) -> some View {
        let fontSize = style?.font_size?.cgFloat ?? defaultSize
        return Text(text)
            .font(.system(size: fontSize, weight: weight))
            .foregroundColor(style?.color?.swiftUIColor)
    }

    // MARK: - Text Helpers

    func getTextContent(_ node: DOMNode) -> String {
        var result = ""
        collectText(node, into: &result)
        return normalizeWhitespace(result)
    }

    func collectText(_ node: DOMNode, into result: inout String) {
        if node.type == "text", let content = node.content {
            result += content
        } else {
            for child in node.childNodes {
                collectText(child, into: &result)
            }
        }
    }

    func normalizeWhitespace(_ text: String) -> String {
        text.components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}

// MARK: - Button Style

struct NavButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: 28, height: 28)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(configuration.isPressed ? Color.gray.opacity(0.2) : Color.clear)
            )
            .foregroundColor(isEnabled ? .primary : .gray.opacity(0.5))
            .contentShape(Rectangle())
    }
}
