// KoalaScreenshot - Headless screenshot tool for Koala Browser
// Renders an HTML file and captures it to a PNG image

import AppKit
import SwiftUI
import KoalaCore

@main
struct KoalaScreenshot {
    static func main() {
        let args = CommandLine.arguments

        guard args.count >= 2 else {
            printUsage()
            exit(1)
        }

        var inputPath: String?
        var outputPath = "screenshot.png"
        var width = 800
        var height = 600

        var i = 1
        while i < args.count {
            switch args[i] {
            case "-o", "--output":
                if i + 1 < args.count {
                    outputPath = args[i + 1]
                    i += 1
                }
            case "-w", "--width":
                if i + 1 < args.count, let w = Int(args[i + 1]) {
                    width = w
                    i += 1
                }
            case "-h", "--height":
                if i + 1 < args.count, let h = Int(args[i + 1]) {
                    height = h
                    i += 1
                }
            case "--help":
                printUsage()
                exit(0)
            default:
                if !args[i].hasPrefix("-") {
                    inputPath = args[i]
                }
            }
            i += 1
        }

        guard let path = inputPath else {
            fputs("Error: No input file specified\n", stderr)
            printUsage()
            exit(1)
        }

        // Resolve path
        let resolvedPath: String
        if path.hasPrefix("/") {
            resolvedPath = path
        } else {
            resolvedPath = FileManager.default.currentDirectoryPath + "/" + path
        }

        // Load and parse HTML
        guard let html = try? String(contentsOfFile: resolvedPath, encoding: .utf8) else {
            fputs("Error: Could not read file '\(resolvedPath)'\n", stderr)
            exit(1)
        }

        guard let dom = KoalaParser.parse(html) else {
            fputs("Error: Could not parse HTML\n", stderr)
            exit(1)
        }

        fputs("Rendering \(resolvedPath) (\(width)x\(height))...\n", stderr)

        // Create the SwiftUI view
        let swiftUIView = DocumentRenderView(node: dom)
            .frame(width: CGFloat(width), height: CGFloat(height))
            .background(Color.white)

        // Use NSHostingView for headless rendering
        let hostingView = NSHostingView(rootView: swiftUIView)
        hostingView.frame = NSRect(x: 0, y: 0, width: width, height: height)

        // Force layout
        hostingView.layoutSubtreeIfNeeded()

        // Create bitmap representation
        guard let bitmapRep = hostingView.bitmapImageRepForCachingDisplay(in: hostingView.bounds) else {
            fputs("Error: Could not create bitmap representation\n", stderr)
            exit(1)
        }

        hostingView.cacheDisplay(in: hostingView.bounds, to: bitmapRep)

        // Create NSImage from bitmap
        let nsImage = NSImage(size: hostingView.bounds.size)
        nsImage.addRepresentation(bitmapRep)

        // Convert to PNG and save
        guard let tiffData = nsImage.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let pngData = bitmap.representation(using: .png, properties: [:]) else {
            fputs("Error: Could not convert image to PNG\n", stderr)
            exit(1)
        }

        do {
            try pngData.write(to: URL(fileURLWithPath: outputPath))
            fputs("Screenshot saved to \(outputPath)\n", stderr)
        } catch {
            fputs("Error: Could not write file: \(error)\n", stderr)
            exit(1)
        }
    }

    static func printUsage() {
        let usage = """
        KoalaScreenshot - Headless screenshot tool

        Usage: koala-screenshot <file> [options]

        Options:
          -o, --output <path>   Output PNG path (default: screenshot.png)
          -w, --width <px>      Viewport width (default: 800)
          -h, --height <px>     Viewport height (default: 600)
          --help                Show this help

        Examples:
          koala-screenshot res/simple.html
          koala-screenshot res/simple.html -o output.png -w 1024 -h 768

        """
        fputs(usage, stderr)
    }
}

// MARK: - Render View (simplified from BrowserView)

struct DocumentRenderView: View {
    let node: DOMNode

    var body: some View {
        ScrollView {
            RenderNodeView(node: node)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

struct RenderNodeView: View {
    let node: DOMNode

    var body: some View {
        switch node.type {
        case "document":
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.childNodes) { child in
                    RenderNodeView(node: child)
                }
            }

        case "element":
            if let tagName = node.tagName {
                RenderElementView(tagName: tagName, node: node)
            }

        case "text":
            if let content = node.content {
                let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    Text(normalizeWhitespace(content))
                }
            }

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

struct RenderElementView: View {
    let tagName: String
    let node: DOMNode

    var style: ComputedStyle? { node.computedStyle }

    var body: some View {
        switch tagName.lowercased() {
        case "html", "head", "title", "meta", "link", "script", "style":
            // Skip or recurse for structural elements
            if tagName.lowercased() == "html" {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(node.childNodes) { child in
                        RenderNodeView(node: child)
                    }
                }
            } else {
                EmptyView()
            }

        case "body":
            VStack(alignment: .leading, spacing: 8) {
                ForEach(node.childNodes) { child in
                    RenderNodeView(node: child)
                }
            }
            .padding(style?.paddingInsets ?? EdgeInsets())
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(style?.background_color?.swiftUIColor ?? Color.clear)
            .foregroundColor(style?.color?.swiftUIColor ?? .primary)

        case "div", "article", "section", "main", "header", "footer":
            VStack(alignment: .leading, spacing: 8) {
                ForEach(node.childNodes) { child in
                    RenderNodeView(node: child)
                }
            }
            .padding(style?.paddingInsets ?? EdgeInsets())
            .background(style?.background_color?.swiftUIColor ?? Color.clear)
            .foregroundColor(style?.color?.swiftUIColor)
            .overlay(borderOverlay)
            .padding(style?.marginInsets ?? EdgeInsets())

        case "h1":
            styledText(defaultSize: 32, weight: .bold)
                .padding(.vertical, 12)

        case "h2":
            styledText(defaultSize: 28, weight: .bold)
                .padding(.vertical, 10)

        case "h3":
            styledText(defaultSize: 24, weight: .semibold)
                .padding(.vertical, 8)

        case "p":
            styledText(defaultSize: 16, weight: .regular)
                .lineSpacing(style?.line_height.map { CGFloat(($0 - 1.0) * 16) } ?? 4)
                .padding(.bottom, style?.margin_bottom?.cgFloat ?? 8)

        case "span":
            if let bgColor = style?.background_color {
                Text(getTextContent(node))
                    .foregroundColor(style?.color?.swiftUIColor)
                    .padding(.horizontal, style?.padding_right?.cgFloat ?? 0)
                    .padding(.vertical, style?.padding_top?.cgFloat ?? 0)
                    .background(bgColor.swiftUIColor)
            } else {
                Text(getTextContent(node))
                    .foregroundColor(style?.color?.swiftUIColor)
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
            Divider().padding(.vertical, 16)

        default:
            VStack(alignment: .leading, spacing: 0) {
                ForEach(node.childNodes) { child in
                    RenderNodeView(node: child)
                }
            }
        }
    }

    @ViewBuilder
    var borderOverlay: some View {
        if let border = style?.border_top {
            RoundedRectangle(cornerRadius: 0)
                .stroke(border.color.swiftUIColor, lineWidth: border.width.cgFloat)
        }
    }

    func styledText(defaultSize: CGFloat, weight: Font.Weight) -> some View {
        let fontSize = style?.font_size?.cgFloat ?? defaultSize
        return Text(getTextContent(node))
            .font(.system(size: fontSize, weight: weight))
            .foregroundColor(style?.color?.swiftUIColor)
    }

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
