//! DOM tree implementation for the Koala browser.
//!
//! This crate provides an arena-based DOM tree structure following the
//! [DOM Living Standard](https://dom.spec.whatwg.org/).
//!
//! # Design
//!
//! The tree uses arena allocation with [`NodeId`] indices for all relationships,
//! providing O(1) access and traversal without borrow checker issues.

use std::collections::{HashMap, HashSet};

/// Map of attribute names to values for an element.
pub type AttributesMap = HashMap<String, String>;

/// A type-safe index into the DOM tree.
///
/// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
/// "Each node has an associated node document..."
///
/// NodeId provides O(1) access to any node in the tree without borrowing issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

impl NodeId {
    /// The root document node is always at index 0.
    pub const ROOT: NodeId = NodeId(0);
}

/// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
///
/// "Node is an abstract interface that is used by all nodes in a tree."
/// "Each node has an associated node document... and parent (null or an element)."
///
/// This node stores indices for parent/child/sibling relationships,
/// enabling O(1) traversal in any direction.
#[derive(Debug, Clone)]
pub struct Node {
    /// "Each node has an associated node type"
    pub node_type: NodeType,

    /// [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-parent)
    /// "An object that participates in a tree has a parent, which is either
    /// null or an object."
    pub parent: Option<NodeId>,

    /// [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-child)
    /// "A node has an associated list of children"
    pub children: Vec<NodeId>,

    /// [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-next-sibling)
    /// "An object A's next sibling is the object immediately following A
    /// in the children of A's parent."
    pub next_sibling: Option<NodeId>,

    /// [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-previous-sibling)
    /// "An object A's previous sibling is the object immediately preceding A
    /// in the children of A's parent."
    pub prev_sibling: Option<NodeId>,
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

/// Arena-based DOM tree with O(1) node access and traversal.
///
/// [§ 4 Nodes](https://dom.spec.whatwg.org/#nodes)
///
/// "The DOM represents a document as a tree. A tree is a finite hierarchical
/// tree structure."
///
/// This structure stores all nodes in a contiguous vector, using indices
/// for all relationships. This provides:
/// - O(1) access to any node by NodeId
/// - O(1) parent/sibling traversal
/// - No borrowing issues (indices instead of references)
/// - Memory-efficient storage
#[derive(Debug, Clone)]
pub struct DomTree {
    /// All nodes in the tree, indexed by NodeId.
    /// The Document node is always at index 0 (NodeId::ROOT).
    nodes: Vec<Node>,
}

impl DomTree {
    /// Create a new DOM tree with just the Document node.
    pub fn new() -> Self {
        let document = Node {
            node_type: NodeType::Document,
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
        };
        DomTree {
            nodes: vec![document],
        }
    }

    /// Get the root document node ID.
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    /// Get a node by its ID.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }

    /// Get the number of nodes in the tree.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the tree is empty (should always have at least the Document).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Allocate a new node and return its ID.
    /// The node is not yet attached to the tree.
    pub fn alloc(&mut self, node_type: NodeType) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            node_type,
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
        });
        id
    }

    /// [§ 4.2.2 Append](https://dom.spec.whatwg.org/#concept-node-append)
    ///
    /// "To append a node to a parent, pre-insert node into parent before null."
    ///
    /// Appends `child` as the last child of `parent`, updating all relationships.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Get the current last child of parent (if any) to set up sibling links
        let prev_last_child = self.nodes[parent.0].children.last().copied();

        // Update parent's children list
        self.nodes[parent.0].children.push(child);

        // Set child's parent
        self.nodes[child.0].parent = Some(parent);

        // Set up sibling links
        if let Some(prev_id) = prev_last_child {
            self.nodes[prev_id.0].next_sibling = Some(child);
            self.nodes[child.0].prev_sibling = Some(prev_id);
        }
    }

    /// Get the parent of a node.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent)
    }

    /// Get all children of a node.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    /// Get the first child of a node.
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.children.first().copied())
    }

    /// Get the last child of a node.
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.children.last().copied())
    }

    /// Get the next sibling of a node.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.next_sibling)
    }

    /// Get the previous sibling of a node.
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.prev_sibling)
    }

    /// [§ 4.2.6 Descendant](https://dom.spec.whatwg.org/#concept-tree-descendant)
    ///
    /// "An object A is called a descendant of an object B, if either A is a
    /// child of B or A is a child of an object C that is a descendant of B."
    ///
    /// Check if `descendant` is a descendant of `ancestor`.
    pub fn is_descendant_of(&self, descendant: NodeId, ancestor: NodeId) -> bool {
        let mut current = self.parent(descendant);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parent(id);
        }
        false
    }

    /// Iterate over all ancestors of a node, from parent to root.
    pub fn ancestors(&self, id: NodeId) -> AncestorIterator<'_> {
        AncestorIterator {
            tree: self,
            current: self.parent(id),
        }
    }

    /// Iterate over preceding siblings (from immediately before to first child).
    pub fn preceding_siblings(&self, id: NodeId) -> PrecedingSiblingIterator<'_> {
        PrecedingSiblingIterator {
            tree: self,
            current: self.prev_sibling(id),
        }
    }

    /// Get element data if this node is an element.
    pub fn as_element(&self, id: NodeId) -> Option<&ElementData> {
        self.get(id).and_then(|n| match &n.node_type {
            NodeType::Element(data) => Some(data),
            _ => None,
        })
    }

    /// Get text content if this node is a text node.
    pub fn as_text(&self, id: NodeId) -> Option<&str> {
        self.get(id).and_then(|n| match &n.node_type {
            NodeType::Text(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// [§ 3.1.1 The document element](https://html.spec.whatwg.org/multipage/dom.html#the-html-element-2)
    ///
    /// "The document element of a document is the element whose parent is that
    /// document, if it exists; otherwise null."
    ///
    /// In practice for HTML documents, this is the `<html>` element.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(NodeId::ROOT)
            .iter()
            .find(|&&id| matches!(self.get(id).map(|n| &n.node_type), Some(NodeType::Element(_))))
            .copied()
    }

    /// [§ 3.1.3 The body element](https://html.spec.whatwg.org/multipage/dom.html#the-body-element-2)
    ///
    /// "The body element of a document is the first of the html element's children
    /// that is either a body element or a frameset element, or null if there is
    /// no such element."
    #[must_use]
    pub fn body(&self) -> Option<NodeId> {
        let html = self.document_element()?;

        self.children(html)
            .iter()
            .find(|&&id| {
                self.as_element(id).is_some_and(|e| {
                    let tag = e.tag_name.to_ascii_lowercase();
                    tag == "body" || tag == "frameset"
                })
            })
            .copied()
    }
}

impl Default for DomTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over ancestors of a node.
pub struct AncestorIterator<'a> {
    tree: &'a DomTree,
    current: Option<NodeId>,
}

impl<'a> Iterator for AncestorIterator<'a> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.tree.parent(id);
        Some(id)
    }
}

/// Iterator over preceding siblings of a node.
pub struct PrecedingSiblingIterator<'a> {
    tree: &'a DomTree,
    current: Option<NodeId>,
}

impl<'a> Iterator for PrecedingSiblingIterator<'a> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.tree.prev_sibling(id);
        Some(id)
    }
}
