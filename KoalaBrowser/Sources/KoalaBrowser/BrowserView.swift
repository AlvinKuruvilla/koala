import SwiftUI

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

            // Theme toggle (placeholder for now)
            Button(action: { }) {
                Image(systemName: "sun.max")
                    .font(.system(size: 14, weight: .medium))
            }
            .buttonStyle(NavButtonStyle())
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

            Text("Enter a file path to load HTML")
                .font(.subheadline)
                .foregroundColor(.secondary)

            Text("Try: res/simple.html")
                .font(.caption)
                .foregroundColor(.gray)
                .padding(.top, 4)
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
        case .document:
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.children) { child in
                    NodeView(node: child)
                }
            }

        case .element(let tagName, _):
            ElementView(tagName: tagName, node: node)

        case .text(let content):
            let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                Text(normalizeWhitespace(content))
            }

        case .comment:
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

    var body: some View {
        switch tagName.lowercased() {
        case "html", "body", "div", "article", "section", "main", "header", "footer", "nav", "aside":
            VStack(alignment: .leading, spacing: 8) {
                ForEach(node.children) { child in
                    NodeView(node: child)
                }
            }

        case "head", "title", "meta", "link", "script", "style":
            EmptyView()

        case "h1":
            Text(getTextContent(node))
                .font(.system(size: 32, weight: .bold))
                .padding(.vertical, 12)

        case "h2":
            Text(getTextContent(node))
                .font(.system(size: 28, weight: .bold))
                .padding(.vertical, 10)

        case "h3":
            Text(getTextContent(node))
                .font(.system(size: 24, weight: .semibold))
                .padding(.vertical, 8)

        case "p":
            Text(getTextContent(node))
                .font(.system(size: 16))
                .lineSpacing(4)
                .padding(.vertical, 8)

        case "span", "a":
            Text(getTextContent(node))

        case "b", "strong":
            Text(getTextContent(node))
                .fontWeight(.bold)

        case "i", "em":
            Text(getTextContent(node))
                .italic()

        case "br":
            Text("\n")

        case "hr":
            Divider()
                .padding(.vertical, 16)

        default:
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.children) { child in
                    NodeView(node: child)
                }
            }
        }
    }

    func getTextContent(_ node: DOMNode) -> String {
        var result = ""
        collectText(node, into: &result)
        return normalizeWhitespace(result)
    }

    func collectText(_ node: DOMNode, into result: inout String) {
        switch node.type {
        case .text(let content):
            result += content
        default:
            for child in node.children {
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
