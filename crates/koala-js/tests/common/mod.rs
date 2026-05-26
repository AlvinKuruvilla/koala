//! Shared fixtures for koala-js integration tests.
//!
//! Each `tests/*.rs` is its own crate, so helpers go under
//! `tests/common/` and are pulled in with `mod common;`. The
//! `common/` subdirectory keeps it from being interpreted as a
//! standalone test file by cargo.

#![allow(dead_code)] // fixtures may be used from only some test files

use std::cell::RefCell;
use std::rc::Rc;

use koala_dom::{AttributesMap, DomTree, ElementData, NodeType};
use koala_js::DomHandle;

/// Minimal fixture: `<html><body><div id="hello" class="greeting prominent"
/// data-track="yes">hi</div></body></html>`. Returns a fresh handle each
/// call so tests can mutate freely without cross-test interference.
pub fn fixture() -> DomHandle {
    let mut tree = DomTree::new();
    let root = tree.root();
    let html = tree.alloc(NodeType::Element(ElementData {
        tag_name: "html".to_string(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(root, html);
    let body = tree.alloc(NodeType::Element(ElementData {
        tag_name: "body".to_string(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(html, body);

    let mut div_attrs = AttributesMap::new();
    let _ = div_attrs.insert("id".to_string(), "hello".to_string());
    let _ = div_attrs.insert("class".to_string(), "greeting prominent".to_string());
    let _ = div_attrs.insert("data-track".to_string(), "yes".to_string());
    let div = tree.alloc(NodeType::Element(ElementData {
        tag_name: "div".to_string(),
        attrs: div_attrs,
    }));
    tree.append_child(body, div);
    let text = tree.alloc(NodeType::Text("hi".into()));
    tree.append_child(div, text);

    Rc::new(RefCell::new(tree))
}

/// Three-item list fixture — `<html><body><ul id="list">` containing
/// `<li id="a">A</li>`, `<li id="b">B</li>`, `<li id="c">C</li>`.
/// Used by tree-nav and selector tests where multiple children
/// matter.
pub fn list_fixture() -> DomHandle {
    let mut tree = DomTree::new();
    let root = tree.root();
    let html = tree.alloc(NodeType::Element(ElementData {
        tag_name: "html".to_string(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(root, html);
    let body = tree.alloc(NodeType::Element(ElementData {
        tag_name: "body".to_string(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(html, body);

    let mut list_attrs = AttributesMap::new();
    let _ = list_attrs.insert("id".into(), "list".into());
    let list = tree.alloc(NodeType::Element(ElementData {
        tag_name: "ul".into(),
        attrs: list_attrs,
    }));
    tree.append_child(body, list);

    for id in ["a", "b", "c"] {
        let mut attrs = AttributesMap::new();
        let _ = attrs.insert("id".into(), id.into());
        let li = tree.alloc(NodeType::Element(ElementData {
            tag_name: "li".into(),
            attrs,
        }));
        tree.append_child(list, li);
        let text = tree.alloc(NodeType::Text(id.to_ascii_uppercase()));
        tree.append_child(li, text);
    }

    Rc::new(RefCell::new(tree))
}

/// `<html><head><title>koala test page</title></head><body></body></html>`.
/// Used by `document.head` and `document.title` tests.
pub fn fixture_with_head() -> DomHandle {
    let mut tree = DomTree::new();
    let root = tree.root();
    let html = tree.alloc(NodeType::Element(ElementData {
        tag_name: "html".into(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(root, html);
    let head = tree.alloc(NodeType::Element(ElementData {
        tag_name: "head".into(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(html, head);
    let title = tree.alloc(NodeType::Element(ElementData {
        tag_name: "title".into(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(head, title);
    let title_text = tree.alloc(NodeType::Text("koala test page".into()));
    tree.append_child(title, title_text);
    let body = tree.alloc(NodeType::Element(ElementData {
        tag_name: "body".into(),
        attrs: AttributesMap::new(),
    }));
    tree.append_child(html, body);
    Rc::new(RefCell::new(tree))
}
