//! Tests for DOM tree mutation methods: remove_child, insert_before, move_children.

use koala_dom::{DomTree, ElementData, NodeId, NodeType};

/// Helper to create an element node and return its NodeId.
fn alloc_element(tree: &mut DomTree, tag: &str) -> NodeId {
    tree.alloc(NodeType::Element(ElementData {
        tag_name: tag.to_string(),
        attrs: Default::default(),
    }))
}

// ========== remove_child ==========

#[test]
fn test_remove_child_single_child() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let child = alloc_element(&mut tree, "p");
    tree.append_child(parent, child);

    assert_eq!(tree.children(parent).len(), 1);

    tree.remove_child(parent, child);

    assert_eq!(tree.children(parent).len(), 0);
    assert_eq!(tree.parent(child), None);
    assert_eq!(tree.prev_sibling(child), None);
    assert_eq!(tree.next_sibling(child), None);
}

#[test]
fn test_remove_child_first_of_three() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let a = alloc_element(&mut tree, "a");
    let b = alloc_element(&mut tree, "b");
    let c = alloc_element(&mut tree, "c");
    tree.append_child(parent, a);
    tree.append_child(parent, b);
    tree.append_child(parent, c);

    tree.remove_child(parent, a);

    // b is now first child, c is second
    assert_eq!(tree.children(parent), &[b, c]);
    assert_eq!(tree.prev_sibling(b), None);
    assert_eq!(tree.next_sibling(b), Some(c));
    assert_eq!(tree.prev_sibling(c), Some(b));
}

#[test]
fn test_remove_child_middle_of_three() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let a = alloc_element(&mut tree, "a");
    let b = alloc_element(&mut tree, "b");
    let c = alloc_element(&mut tree, "c");
    tree.append_child(parent, a);
    tree.append_child(parent, b);
    tree.append_child(parent, c);

    tree.remove_child(parent, b);

    // a and c are siblings now
    assert_eq!(tree.children(parent), &[a, c]);
    assert_eq!(tree.next_sibling(a), Some(c));
    assert_eq!(tree.prev_sibling(c), Some(a));
}

#[test]
fn test_remove_child_last_of_three() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let a = alloc_element(&mut tree, "a");
    let b = alloc_element(&mut tree, "b");
    let c = alloc_element(&mut tree, "c");
    tree.append_child(parent, a);
    tree.append_child(parent, b);
    tree.append_child(parent, c);

    tree.remove_child(parent, c);

    assert_eq!(tree.children(parent), &[a, b]);
    assert_eq!(tree.next_sibling(b), None);
}

// ========== insert_before ==========

#[test]
fn test_insert_before_first_child() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let existing = alloc_element(&mut tree, "b");
    tree.append_child(parent, existing);

    let new_child = alloc_element(&mut tree, "a");
    tree.insert_before(parent, new_child, existing);

    // new_child should be first, existing second
    assert_eq!(tree.children(parent), &[new_child, existing]);
    assert_eq!(tree.parent(new_child), Some(parent));
    assert_eq!(tree.next_sibling(new_child), Some(existing));
    assert_eq!(tree.prev_sibling(new_child), None);
    assert_eq!(tree.prev_sibling(existing), Some(new_child));
}

#[test]
fn test_insert_before_middle() {
    let mut tree = DomTree::new();
    let parent = alloc_element(&mut tree, "div");
    tree.append_child(NodeId::ROOT, parent);

    let a = alloc_element(&mut tree, "a");
    let c = alloc_element(&mut tree, "c");
    tree.append_child(parent, a);
    tree.append_child(parent, c);

    let b = alloc_element(&mut tree, "b");
    tree.insert_before(parent, b, c);

    assert_eq!(tree.children(parent), &[a, b, c]);
    assert_eq!(tree.next_sibling(a), Some(b));
    assert_eq!(tree.prev_sibling(b), Some(a));
    assert_eq!(tree.next_sibling(b), Some(c));
    assert_eq!(tree.prev_sibling(c), Some(b));
}

// ========== move_children ==========

#[test]
fn test_move_children_basic() {
    let mut tree = DomTree::new();
    let from = alloc_element(&mut tree, "div");
    let to = alloc_element(&mut tree, "span");
    tree.append_child(NodeId::ROOT, from);
    tree.append_child(NodeId::ROOT, to);

    let a = alloc_element(&mut tree, "a");
    let b = alloc_element(&mut tree, "b");
    tree.append_child(from, a);
    tree.append_child(from, b);

    tree.move_children(from, to);

    // from should be empty
    assert_eq!(tree.children(from).len(), 0);
    // to should have both children
    assert_eq!(tree.children(to), &[a, b]);
    assert_eq!(tree.parent(a), Some(to));
    assert_eq!(tree.parent(b), Some(to));
}

#[test]
fn test_move_children_appends_to_existing() {
    let mut tree = DomTree::new();
    let from = alloc_element(&mut tree, "div");
    let to = alloc_element(&mut tree, "span");
    tree.append_child(NodeId::ROOT, from);
    tree.append_child(NodeId::ROOT, to);

    let existing = alloc_element(&mut tree, "x");
    tree.append_child(to, existing);

    let moved = alloc_element(&mut tree, "y");
    tree.append_child(from, moved);

    tree.move_children(from, to);

    assert_eq!(tree.children(to), &[existing, moved]);
    // Sibling links between existing and moved
    assert_eq!(tree.next_sibling(existing), Some(moved));
    assert_eq!(tree.prev_sibling(moved), Some(existing));
}

#[test]
fn test_move_children_empty_source() {
    let mut tree = DomTree::new();
    let from = alloc_element(&mut tree, "div");
    let to = alloc_element(&mut tree, "span");
    tree.append_child(NodeId::ROOT, from);
    tree.append_child(NodeId::ROOT, to);

    // Moving no children should be a no-op
    tree.move_children(from, to);

    assert_eq!(tree.children(from).len(), 0);
    assert_eq!(tree.children(to).len(), 0);
}
