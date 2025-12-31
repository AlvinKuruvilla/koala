// KoalaCore - Swift wrapper for the Koala Rust engine
// This module provides Swift-friendly access to the Rust HTML/CSS parser

import Foundation
import CKoala

/// Swift wrapper for the Koala HTML parser (Rust FFI)
public class KoalaParser {

    /// Parse HTML string and return a JSON representation of the DOM
    /// - Parameter html: The HTML string to parse
    /// - Returns: A JSON string representing the DOM tree, or nil on error
    public static func parseHTML(_ html: String) -> String? {
        guard let doc = koala_parse_html(html) else {
            return nil
        }
        defer { koala_document_free(doc) }

        guard let jsonPtr = koala_document_to_json(doc) else {
            return nil
        }
        defer { koala_string_free(jsonPtr) }

        return String(cString: jsonPtr)
    }

    /// Parse HTML and decode into a DOMNode structure
    /// - Parameter html: The HTML string to parse
    /// - Returns: The root DOMNode, or nil on error
    public static func parse(_ html: String) -> DOMNode? {
        guard let json = parseHTML(html) else {
            return nil
        }

        guard let data = json.data(using: .utf8) else {
            return nil
        }

        do {
            let decoder = JSONDecoder()
            return try decoder.decode(DOMNode.self, from: data)
        } catch {
            print("[KoalaCore] Failed to decode DOM: \(error)")
            return nil
        }
    }
}

// MARK: - CSS Value Types

import SwiftUI

/// CSS color value (RGBA)
public struct CSSColor: Decodable {
    public let r: UInt8
    public let g: UInt8
    public let b: UInt8
    public let a: UInt8

    /// Convert to SwiftUI Color
    public var swiftUIColor: Color {
        Color(
            red: Double(r) / 255.0,
            green: Double(g) / 255.0,
            blue: Double(b) / 255.0,
            opacity: Double(a) / 255.0
        )
    }
}

/// CSS length value
public enum CSSLength: Decodable {
    case px(Double)

    /// Get value in points (for SwiftUI)
    public var cgFloat: CGFloat {
        switch self {
        case .px(let value):
            return CGFloat(value)
        }
    }

    // Custom decoding for tagged enum from Rust
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let px = try container.decodeIfPresent(Double.self, forKey: .Px) {
            self = .px(px)
        } else {
            self = .px(0)
        }
    }

    private enum CodingKeys: String, CodingKey {
        case Px
    }
}

/// CSS border value
public struct CSSBorder: Decodable {
    public let width: CSSLength
    public let style: String
    public let color: CSSColor
}

/// Computed CSS styles for an element
public struct ComputedStyle: Decodable {
    // Text properties
    public let color: CSSColor?
    public let font_family: String?
    public let font_size: CSSLength?
    public let line_height: Double?

    // Background
    public let background_color: CSSColor?

    // Box model - margins
    public let margin_top: CSSLength?
    public let margin_right: CSSLength?
    public let margin_bottom: CSSLength?
    public let margin_left: CSSLength?

    // Box model - padding
    public let padding_top: CSSLength?
    public let padding_right: CSSLength?
    public let padding_bottom: CSSLength?
    public let padding_left: CSSLength?

    // Box model - borders
    public let border_top: CSSBorder?
    public let border_right: CSSBorder?
    public let border_bottom: CSSBorder?
    public let border_left: CSSBorder?

    /// Get EdgeInsets for padding
    public var paddingInsets: EdgeInsets {
        EdgeInsets(
            top: padding_top?.cgFloat ?? 0,
            leading: padding_left?.cgFloat ?? 0,
            bottom: padding_bottom?.cgFloat ?? 0,
            trailing: padding_right?.cgFloat ?? 0
        )
    }

    /// Get EdgeInsets for margin
    public var marginInsets: EdgeInsets {
        EdgeInsets(
            top: margin_top?.cgFloat ?? 0,
            leading: margin_left?.cgFloat ?? 0,
            bottom: margin_bottom?.cgFloat ?? 0,
            trailing: margin_right?.cgFloat ?? 0
        )
    }
}

// MARK: - DOM Node

/// DOM Node representation matching the Rust structure
public class DOMNode: Decodable, Identifiable {
    public let id = UUID()
    public let type: String
    public let tagName: String?
    public let attributes: [String: String]?
    public let content: String?
    public let children: [DOMNode]?
    public let computedStyle: ComputedStyle?

    private enum CodingKeys: String, CodingKey {
        case type, tagName, attributes, content, children, computedStyle
    }

    public required init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        type = try container.decode(String.self, forKey: .type)
        tagName = try container.decodeIfPresent(String.self, forKey: .tagName)
        attributes = try container.decodeIfPresent([String: String].self, forKey: .attributes)
        content = try container.decodeIfPresent(String.self, forKey: .content)
        children = try container.decodeIfPresent([DOMNode].self, forKey: .children)
        computedStyle = try container.decodeIfPresent(ComputedStyle.self, forKey: .computedStyle)
    }

    /// Get all child nodes (empty array if none)
    public var childNodes: [DOMNode] {
        children ?? []
    }
}
