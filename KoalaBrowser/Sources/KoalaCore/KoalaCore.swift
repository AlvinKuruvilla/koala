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

/// DOM Node representation matching the Rust structure
public class DOMNode: Decodable, Identifiable {
    public let id = UUID()
    public let type: String
    public let tagName: String?
    public let attributes: [String: String]?
    public let content: String?
    public let children: [DOMNode]?

    private enum CodingKeys: String, CodingKey {
        case type, tagName, attributes, content, children
    }

    public required init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        type = try container.decode(String.self, forKey: .type)
        tagName = try container.decodeIfPresent(String.self, forKey: .tagName)
        attributes = try container.decodeIfPresent([String: String].self, forKey: .attributes)
        content = try container.decodeIfPresent(String.self, forKey: .content)
        children = try container.decodeIfPresent([DOMNode].self, forKey: .children)
    }

    /// Get all child nodes (empty array if none)
    public var childNodes: [DOMNode] {
        children ?? []
    }
}
