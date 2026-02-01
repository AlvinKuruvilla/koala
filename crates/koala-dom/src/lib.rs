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
///
/// [§ 4.9.1 Interface Attr](https://dom.spec.whatwg.org/#interface-attr)
/// "An Attr object represents an attribute of an Element object."
///
/// [§ 4.9.2 Interface `NamedNodeMap`](https://dom.spec.whatwg.org/#interface-namednodemap)
/// "A `NamedNodeMap` has an associated element (an element)."
///
/// NOTE: This is a simplified representation. Full DOM spec compliance would require:
/// - Namespace URI and namespace prefix per attribute
/// - Attr node objects with ownerElement references
/// - `NamedNodeMap` interface with getNamedItem/setNamedItem methods
///
/// We use a simple String->String map since we don't currently need namespace support.
pub type AttributesMap = HashMap<String, String>;

/// A type-safe index into the DOM tree.
///
/// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
/// "Each node has an associated node document..."
///
/// `NodeId` provides O(1) access to any node in the tree without borrowing issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

impl NodeId {
    /// The root document node is always at index 0.
    pub const ROOT: Self = Self(0);
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
///   custom element definition, is value."
/// - "When an element is created, its local name is always given."
///
/// NOTE: We only store `tag_name` (local name) and attrs for simplicity.
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
    #[must_use]
    pub fn id(&self) -> Option<&String> {
        self.attrs.get("id")
    }

    /// Returns the set of class names from the class attribute.
    ///
    /// Per [§ 3.2.6 Global attributes](https://html.spec.whatwg.org/multipage/dom.html#global-attributes):
    /// "The class attribute, if specified, must have a value that is a set of
    /// space-separated tokens representing the various classes that the element belongs to."
    #[must_use]
    pub fn classes(&self) -> HashSet<&str> {
        self.attrs.get("class").map_or_else(HashSet::new, |classlist| classlist.split(' ').collect())
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
/// - O(1) access to any node by `NodeId`
/// - O(1) parent/sibling traversal
/// - No borrowing issues (indices instead of references)
/// - Memory-efficient storage
#[derive(Debug, Clone)]
pub struct DomTree {
    /// All nodes in the tree, indexed by `NodeId`.
    /// The Document node is always at index 0 (`NodeId::ROOT`).
    nodes: Vec<Node>,
}

impl DomTree {
    /// [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
    ///
    /// "A document is created by the 'create a document' algorithm."
    ///
    /// Create a new DOM tree with just the Document node at the root.
    /// The Document node is always at index 0 (`NodeId::ROOT`).
    #[must_use]
    pub fn new() -> Self {
        // STEP 1: Create the Document node.
        // [§ 4.5](https://dom.spec.whatwg.org/#interface-document)
        //
        // "A document has an associated type... which is 'html' for HTML documents."
        let document = Node {
            node_type: NodeType::Document,
            // STEP 2: Initialize all relationships to None.
            // The Document has no parent (it is the root of the tree).
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
        };

        // STEP 3: Place Document at index 0 (`NodeId::ROOT`).
        Self {
            nodes: vec![document],
        }
    }

    /// Get the root document node ID.
    #[must_use]
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    /// Get a node by its ID.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }

    /// Get the number of nodes in the tree.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the tree is empty (should always have at least the Document).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
    ///
    /// Allocate a new node in the arena and return its ID.
    /// The node is not yet attached to the tree (no parent, no siblings).
    ///
    /// NOTE: This is an implementation detail of our arena-based tree.
    /// The DOM spec doesn't have an explicit "allocate" concept; nodes are
    /// created and inserted in a single operation. We separate these for
    /// flexibility in tree construction.
    pub fn alloc(&mut self, node_type: NodeType) -> NodeId {
        // STEP 1: Assign the next available index as the `NodeId`.
        let id = NodeId(self.nodes.len());

        // STEP 2: Create the node with no relationships.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-parent)
        //
        // "An object that participates in a tree has a parent, which is
        // either null or an object."
        //
        // Initially, parent and siblings are null until the node is inserted.
        self.nodes.push(Node {
            node_type,
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
        });

        // STEP 3: Return the `NodeId` for later insertion.
        id
    }

    /// [§ 4.2.2 Append](https://dom.spec.whatwg.org/#concept-node-append)
    ///
    /// "To append a node to a parent, pre-insert node into parent before null."
    ///
    /// [§ 4.2.1 Insert](https://dom.spec.whatwg.org/#concept-node-insert)
    ///
    /// The insert algorithm updates parent/child/sibling relationships.
    /// This simplified implementation handles the common case of appending
    /// to the end of the children list.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // STEP 1: Find the previous last child (if any) for sibling linking.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-child)
        //
        // "An object's last child is its last inclusive descendant that is
        // also its child, or null if it has no children."
        let prev_last_child = self.nodes[parent.0].children.last().copied();

        // STEP 2: Add child to parent's children list.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-child)
        //
        // "An object A is called a child of an object B if B is A's parent."
        self.nodes[parent.0].children.push(child);

        // STEP 3: Set child's parent pointer.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-parent)
        //
        // "An object that participates in a tree has a parent..."
        self.nodes[child.0].parent = Some(parent);

        // STEP 4: Update sibling links if there was a previous last child.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-next-sibling)
        //
        // "An object A's next sibling is the object immediately following A
        // in the children of A's parent."
        if let Some(prev_id) = prev_last_child {
            // The previous last child's next_sibling now points to the new child.
            self.nodes[prev_id.0].next_sibling = Some(child);
            // The new child's prev_sibling points back to the previous last child.
            self.nodes[child.0].prev_sibling = Some(prev_id);
        }
        // NOTE: If there was no previous child, child.prev_sibling remains None.
    }

    /// Get the parent of a node.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent)
    }

    /// Get all children of a node.
    #[must_use]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(id).map_or(&[], |n| n.children.as_slice())
    }

    /// Get the first child of a node.
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.children.first().copied())
    }

    /// Get the last child of a node.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.children.last().copied())
    }

    /// Get the next sibling of a node.
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.next_sibling)
    }

    /// Get the previous sibling of a node.
    #[must_use]
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.prev_sibling)
    }

    /// [§ 4.2.6 Descendant](https://dom.spec.whatwg.org/#concept-tree-descendant)
    ///
    /// "An object A is called a descendant of an object B, if either A is a
    /// child of B or A is a child of an object C that is a descendant of B."
    ///
    /// Check if `descendant` is a descendant of `ancestor` by walking up
    /// the parent chain.
    #[must_use]
    pub fn is_descendant_of(&self, descendant: NodeId, ancestor: NodeId) -> bool {
        // STEP 1: Start at the descendant's parent.
        // [§ 4.4](https://dom.spec.whatwg.org/#concept-tree-parent)
        //
        // "An object that participates in a tree has a parent..."
        let mut current = self.parent(descendant);

        // STEP 2: Walk up the tree comparing each ancestor.
        // Per the spec definition, A is a descendant of B if:
        // - "A is a child of B" (direct parent), OR
        // - "A is a child of an object C that is a descendant of B" (transitive)
        while let Some(id) = current {
            // STEP 2a: Check if we've found the target ancestor.
            if id == ancestor {
                return true;
            }
            // STEP 2b: Move up to the next parent.
            current = self.parent(id);
        }

        // STEP 3: Reached root without finding ancestor.
        false
    }

    /// [§ 4.2.5 Ancestor](https://dom.spec.whatwg.org/#concept-tree-ancestor)
    ///
    /// "An object A is called an ancestor of an object B if and only if B is
    /// a descendant of A."
    ///
    /// Returns an iterator over all ancestors of a node, from parent to root.
    /// The iterator yields `NodeId`s in order: parent, grandparent, ..., root.
    #[must_use]
    pub fn ancestors(&self, id: NodeId) -> AncestorIterator<'_> {
        AncestorIterator {
            tree: self,
            // Start at the node's parent (the first ancestor).
            current: self.parent(id),
        }
    }

    /// [§ 4.2.5 Previous sibling](https://dom.spec.whatwg.org/#concept-tree-previous-sibling)
    ///
    /// "An object A's previous sibling is the object immediately preceding A
    /// in the children of A's parent."
    ///
    /// Returns an iterator over preceding siblings, from immediately before
    /// to the first child of the parent.
    #[must_use]
    pub fn preceding_siblings(&self, id: NodeId) -> PrecedingSiblingIterator<'_> {
        PrecedingSiblingIterator {
            tree: self,
            // Start at the node's previous sibling.
            current: self.prev_sibling(id),
        }
    }

    /// Get element data if this node is an element.
    #[must_use]
    pub fn as_element(&self, id: NodeId) -> Option<&ElementData> {
        self.get(id).and_then(|n| match &n.node_type {
            NodeType::Element(data) => Some(data),
            _ => None,
        })
    }

    /// Get text content if this node is a text node.
    #[must_use]
    pub fn as_text(&self, id: NodeId) -> Option<&str> {
        self.get(id).and_then(|n| match &n.node_type {
            NodeType::Text(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// [§ 4.2.6 Descendant](https://dom.spec.whatwg.org/#concept-tree-descendant)
    ///
    /// "An object A is called a descendant of an object B, if either A is a
    /// child of B or A is a child of an object C that is a descendant of B."
    ///
    /// Returns an iterator over all descendants of a node in document order
    /// (depth-first, pre-order traversal). Does not include the starting node.
    #[must_use]
    pub fn descendants(&self, id: NodeId) -> DescendantIterator<'_> {
        DescendantIterator {
            tree: self,
            // Start with the first child of the given node
            stack: self.children(id).iter().rev().copied().collect(),
        }
    }

    /// Iterate over all nodes in the tree in document order.
    ///
    /// [§ 4.2.4 Tree order](https://dom.spec.whatwg.org/#concept-tree-order)
    ///
    /// "An object A is preceding an object B if A and B are in the same tree
    /// and A comes before B in tree order."
    ///
    /// This returns a depth-first, pre-order traversal starting from the root,
    /// which matches the document order defined by the spec.
    pub fn iter_all(&self) -> impl Iterator<Item = NodeId> + '_ {
        // Include the root node, then all its descendants
        std::iter::once(self.root()).chain(self.descendants(self.root()))
    }

    /// [§ 3.1.1 The document element](https://html.spec.whatwg.org/multipage/dom.html#the-html-element-2)
    ///
    /// "The document element of a document is the element whose parent is that
    /// document, if it exists; otherwise null."
    ///
    /// In practice for HTML documents, this is the `<html>` element.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        // STEP 1: Get children of the Document node.
        // [§ 4.5](https://dom.spec.whatwg.org/#interface-document)
        //
        // The Document may have children like DOCTYPE, comments, and the
        // document element (html).
        let children = self.children(NodeId::ROOT);

        // STEP 2: Find the first Element child.
        // [§ 3.1.1](https://html.spec.whatwg.org/multipage/dom.html#the-html-element-2)
        //
        // "The document element of a document is the element whose parent
        // is that document, if it exists; otherwise null."
        //
        // NOTE: Per spec, there should be at most one Element child of Document.
        // DOCTYPE and Comment nodes are not Elements.
        children
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
        // STEP 1: Get the document element (html).
        // [§ 3.1.1](https://html.spec.whatwg.org/multipage/dom.html#the-html-element-2)
        //
        // "The document element of a document is the element whose parent
        // is that document..."
        let html = self.document_element()?;

        // STEP 2: Search html's children for body or frameset.
        // [§ 3.1.3](https://html.spec.whatwg.org/multipage/dom.html#the-body-element-2)
        //
        // "The body element of a document is the first of the html element's
        // children that is either a body element or a frameset element..."
        //
        // NOTE: The spec says "first", so we iterate in document order and
        // return the first match.
        self.children(html)
            .iter()
            .find(|&&id| {
                self.as_element(id).is_some_and(|e| {
                    // [§ 4.3.1](https://html.spec.whatwg.org/multipage/sections.html#the-body-element)
                    // [§ 15.6](https://html.spec.whatwg.org/multipage/obsolete.html#frameset)
                    //
                    // Tag names are compared case-insensitively for HTML elements.
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

/// [§ 4.2.5 Ancestor](https://dom.spec.whatwg.org/#concept-tree-ancestor)
///
/// Iterator that walks up the tree from a node to the root.
/// Each call to `next()` returns the next ancestor in the chain.
pub struct AncestorIterator<'a> {
    tree: &'a DomTree,
    current: Option<NodeId>,
}

impl Iterator for AncestorIterator<'_> {
    type Item = NodeId;

    /// [§ 4.2.5](https://dom.spec.whatwg.org/#concept-tree-ancestor)
    ///
    /// Return the current ancestor and advance to its parent.
    /// Terminates when we reach a node with no parent (the root).
    fn next(&mut self) -> Option<Self::Item> {
        // STEP 1: Return current node if any.
        let id = self.current?;
        // STEP 2: Advance to the next ancestor (parent of current).
        self.current = self.tree.parent(id);
        Some(id)
    }
}

/// [§ 4.2.5 Previous sibling](https://dom.spec.whatwg.org/#concept-tree-previous-sibling)
///
/// Iterator that walks backwards through siblings of a node.
/// Each call to `next()` returns the next preceding sibling.
pub struct PrecedingSiblingIterator<'a> {
    tree: &'a DomTree,
    current: Option<NodeId>,
}

impl Iterator for PrecedingSiblingIterator<'_> {
    type Item = NodeId;

    /// [§ 4.2.5](https://dom.spec.whatwg.org/#concept-tree-previous-sibling)
    ///
    /// Return the current sibling and advance to its previous sibling.
    /// Terminates when we reach the first child (no previous sibling).
    fn next(&mut self) -> Option<Self::Item> {
        // STEP 1: Return current sibling if any.
        let id = self.current?;
        // STEP 2: Advance to the previous sibling.
        self.current = self.tree.prev_sibling(id);
        Some(id)
    }
}

/// [§ 4.2.6 Descendant](https://dom.spec.whatwg.org/#concept-tree-descendant)
///
/// Iterator that walks the tree in document order (depth-first, pre-order).
/// Each call to `next()` returns the next descendant of the starting node.
pub struct DescendantIterator<'a> {
    tree: &'a DomTree,
    /// Stack of nodes to visit (children are pushed in reverse order so we
    /// process them left-to-right).
    stack: Vec<NodeId>,
}

impl Iterator for DescendantIterator<'_> {
    type Item = NodeId;

    /// [§ 4.2.4 Tree order](https://dom.spec.whatwg.org/#concept-tree-order)
    ///
    /// Return the next node in tree order and push its children onto the stack.
    fn next(&mut self) -> Option<Self::Item> {
        // STEP 1: Pop the next node from the stack.
        let id = self.stack.pop()?;

        // STEP 2: Push all children onto the stack in reverse order.
        // This ensures left-to-right traversal when we pop from the stack.
        let children = self.tree.children(id);
        self.stack.extend(children.iter().rev().copied());

        // STEP 3: Return the current node.
        Some(id)
    }
}
