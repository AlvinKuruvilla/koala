use std::collections::{HashMap, HashSet};

pub type AttributesMap = HashMap<String, String>;

/// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
///
/// "Node is an abstract interface that is used by all nodes in a tree."
/// "Each node has an associated node document... and parent (null or an element)."
///
/// NOTE: We use `children: Vec<Node>` instead of the spec's parent/sibling pointers
/// for simplicity. This is a tree, not a graph.
#[derive(Debug, Clone)]
pub struct Node {
    /// "A node has an associated list of children"
    pub children: Vec<Node>,
    /// "Each node has an associated node type"
    pub node_type: NodeType,
}

/// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
///
/// "Each node has an associated node type"
#[derive(Debug, Clone)]
pub enum NodeType {
    /// [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
    /// "A document whose type is "html" is known as an HTML document."
    Document,
    /// [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
    /// "Element nodes are simply known as elements."
    Element(ElementData),
    /// [§ 4.10 Interface Text](https://dom.spec.whatwg.org/#interface-text)
    /// "Text nodes are known as text."
    Text(String),
    /// [§ 4.7 Interface Comment](https://dom.spec.whatwg.org/#interface-comment)
    /// "Comment nodes are known as comments."
    Comment(String),
}

/// Element-specific data.
///
/// Per [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element):
/// - "Elements have an associated namespace, namespace prefix, local name, custom element state,
///    custom element definition, is value."
/// - "When an element is created, its local name is always given."
///
/// NOTE: We only store tag_name (local name) and attrs for simplicity.
/// Full spec compliance would require namespace handling, custom elements, etc.
#[derive(Debug, Clone)]
pub struct ElementData {
    /// "An element's local name"
    pub tag_name: String,
    /// "An element has an associated attribute list"
    pub attrs: AttributesMap,
}

// Constructor functions for convenience:

pub fn text(data: String) -> Node {
    Node {
        children: vec![],
        node_type: NodeType::Text(data),
    }
}

pub fn elem(tag_name: String, attrs: AttributesMap, children: Vec<Node>) -> Node {
    Node {
        children,
        node_type: NodeType::Element(ElementData { tag_name, attrs }),
    }
}

pub fn comment(data: String) -> Node {
    Node {
        children: vec![],
        node_type: NodeType::Comment(data),
    }
}
impl ElementData {
    /// Returns the element's id attribute value if present.
    ///
    /// Per [§ 3.2.6 Global attributes](https://html.spec.whatwg.org/multipage/dom.html#global-attributes):
    /// "The id attribute specifies its element's unique identifier (ID)."
    pub fn id(&self) -> Option<&String> {
        self.attrs.get("id")
    }

    /// Returns the set of class names from the class attribute.
    ///
    /// Per [§ 3.2.6 Global attributes](https://html.spec.whatwg.org/multipage/dom.html#global-attributes):
    /// "The class attribute, if specified, must have a value that is a set of
    /// space-separated tokens representing the various classes that the element belongs to."
    pub fn classes(&self) -> HashSet<&str> {
        match self.attrs.get("class") {
            Some(classlist) => classlist.split(' ').collect(),
            None => HashSet::new(),
        }
    }
}
