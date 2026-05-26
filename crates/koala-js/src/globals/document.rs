//! `document` global — Phase-2 DOM bridge entry point.
//!
//! [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//!
//! "The Document interface represents any web page loaded in the
//! browser and serves as an entry point into the web page's
//! content, which is the DOM tree."
//!
//! The exposed methods read and mutate the DOM through
//! [`crate::dom_handle::with_dom`] / [`crate::dom_handle::with_dom_mut`],
//! which find the document's tree via the thread-local that
//! [`JsRuntime::execute`] installs.
//!
//! [`JsRuntime::execute`]: crate::JsRuntime::execute
//!
//! This file is intentionally thin: it just owns the
//! `document.*` surface (methods + accessors) and dispatches into
//! the wrappers in sibling modules:
//!
//! - [`super::element`] — Element wrapper + every `Element.*`
//!   method and accessor
//! - [`super::text`] — Text wrapper
//! - [`super::selectors`] — `querySelector` / `querySelectorAll`
//!   plumbing shared with Element
//! - [`super::helpers`] — error constructors, node-id IO, etc.

use boa_engine::{
    Context, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};
use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};

use crate::dom_handle::{with_dom, with_dom_mut};

use super::element::{array_of_element_objects, make_element_object};
use super::helpers::{
    descendant_text, getter, js_string_value, no_dom_error, required_string_arg,
};
use super::selectors::{find_all_matches, find_first_match, parse_query_arg};
use super::text::make_text_object;

/// Register the `document` global. Called once by
/// [`super::register_globals`].
pub fn register_document(context: &mut Context) {
    let body_getter = getter(context, document_body_get);
    let head_getter = getter(context, document_head_get);
    let document_element_getter = getter(context, document_document_element_get);
    let title_getter = getter(context, document_title_get);

    let accessor_attrs = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;

    let document = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_copy_closure(get_element_by_id),
            js_string!("getElementById"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(get_elements_by_tag_name),
            js_string!("getElementsByTagName"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(get_elements_by_class_name),
            js_string!("getElementsByClassName"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(create_element),
            js_string!("createElement"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(create_text_node),
            js_string!("createTextNode"),
            1,
        )
        .accessor(js_string!("body"), Some(body_getter), None, accessor_attrs)
        .accessor(js_string!("head"), Some(head_getter), None, accessor_attrs)
        .accessor(
            js_string!("documentElement"),
            Some(document_element_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("title"),
            Some(title_getter),
            None,
            accessor_attrs,
        )
        .build();

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("`document` global should not already exist");
}

/// `document.getElementById(elementId)` — first element in tree
/// order whose `id` attribute equals `elementId`, or `null`.
///
/// [§ 5.1 NonElementParentNode.getElementById](https://dom.spec.whatwg.org/#dom-nonelementparentnode-getelementbyid)
fn get_element_by_id(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let id_arg = required_string_arg(args, 0, "getElementById", "id", context)?;

    // STEP 1: "If elementId is the empty string, return null."
    if id_arg.is_empty() {
        return Ok(JsValue::null());
    }

    let node = with_dom(|dom| {
        dom.iter_all().find(|&id| {
            dom.as_element(id)
                .and_then(|e| e.id())
                .is_some_and(|got| got == &id_arg)
        })
    })
    .flatten();

    match node {
        Some(node_id) => make_element_object(context, node_id),
        None => Ok(JsValue::null()),
    }
}

/// `document.querySelector(selectors)` — first descendant of the
/// document that matches `selectors`, or `null`.
///
/// [§ 4.2.6 ParentNode.querySelector](https://dom.spec.whatwg.org/#dom-parentnode-queryselector)
fn query_selector(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = parse_query_arg(args, "querySelector", context)?;
    let Some(parsed) = parsed else { return Ok(JsValue::null()) };

    let result = with_dom(|dom| {
        let root = dom.document_element()?;
        find_first_match(dom, root, &parsed)
    })
    .flatten();

    match result {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

/// `document.querySelectorAll(selectors)` — array of every
/// descendant of the document matching `selectors`.
///
/// [§ 4.2.6 ParentNode.querySelectorAll](https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall)
fn query_selector_all(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = parse_query_arg(args, "querySelectorAll", context)?;
    let Some(parsed) = parsed else {
        return Ok(boa_engine::object::builtins::JsArray::new(context).into());
    };

    let ids: Vec<NodeId> = with_dom(|dom| {
        dom.document_element()
            .map(|root| find_all_matches(dom, root, &parsed))
            .unwrap_or_default()
    })
    .unwrap_or_default();

    array_of_element_objects(ids, context)
}

/// `document.getElementsByTagName(qualifiedName)` — all elements
/// in the document with the given tag name. `"*"` matches every
/// element. Tag name comparison is ASCII case-insensitive for HTML
/// documents.
///
/// [§ 4.5 Document.getElementsByTagName](https://dom.spec.whatwg.org/#dom-document-getelementsbytagname)
fn get_elements_by_tag_name(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let name = required_string_arg(args, 0, "getElementsByTagName", "qualifiedName", context)?;
    let wildcard = name == "*";
    let needle = name.to_ascii_lowercase();

    let ids: Vec<NodeId> = with_dom(|dom| {
        dom.iter_all()
            .filter(|&id| {
                dom.as_element(id)
                    .is_some_and(|e| wildcard || e.tag_name.eq_ignore_ascii_case(&needle))
            })
            .collect()
    })
    .unwrap_or_default();

    array_of_element_objects(ids, context)
}

/// `document.getElementsByClassName(classNames)` — all elements
/// whose class set contains *every* class in the space-separated
/// `classNames` argument.
///
/// [§ 4.5 Document.getElementsByClassName](https://dom.spec.whatwg.org/#dom-document-getelementsbyclassname)
fn get_elements_by_class_name(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let raw = required_string_arg(args, 0, "getElementsByClassName", "classNames", context)?;
    let needle: Vec<String> = raw.split_whitespace().map(str::to_string).collect();
    if needle.is_empty() {
        return Ok(boa_engine::object::builtins::JsArray::new(context).into());
    }

    let ids: Vec<NodeId> = with_dom(|dom| {
        dom.iter_all()
            .filter(|&id| {
                dom.as_element(id).is_some_and(|e| {
                    let classes = e.classes();
                    needle.iter().all(|c| classes.contains(c.as_str()))
                })
            })
            .collect()
    })
    .unwrap_or_default();

    array_of_element_objects(ids, context)
}

/// `document.createElement(localName)` — allocate a new Element
/// node in the DOM and return its wrapper. The element is NOT
/// attached to any parent; use `Element.appendChild` to insert it.
///
/// [§ 4.5 Document.createElement](https://dom.spec.whatwg.org/#dom-document-createelement)
fn create_element(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let name = required_string_arg(args, 0, "createElement", "localName", context)?
        .to_ascii_lowercase();

    let new_id = with_dom_mut(|dom| {
        dom.alloc(NodeType::Element(ElementData {
            tag_name: name,
            attrs: AttributesMap::new(),
        }))
    })
    .ok_or_else(no_dom_error)?;

    make_element_object(context, new_id)
}

/// `document.createTextNode(data)` — allocate a new Text node and
/// return a minimal wrapper. Pass it to `Element.appendChild` to
/// insert it into the tree.
///
/// [§ 4.5 Document.createTextNode](https://dom.spec.whatwg.org/#dom-document-createtextnode)
fn create_text_node(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let data = required_string_arg(args, 0, "createTextNode", "data", context)?;
    let new_id = with_dom_mut(|dom| dom.alloc(NodeType::Text(data))).ok_or_else(no_dom_error)?;
    make_text_object(context, new_id)
}

// ---- document accessors ----

fn document_body_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let body = with_dom(DomTree::body).flatten();
    match body {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn document_head_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let head = with_dom(find_head).flatten();
    match head {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn document_document_element_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let html = with_dom(DomTree::document_element).flatten();
    match html {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

#[allow(clippy::unnecessary_wraps)] // NativeFunction callback shape
fn document_title_get(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let title = with_dom(|dom| {
        let title_id = dom.iter_all().find(|&id| {
            dom.as_element(id)
                .is_some_and(|e| e.tag_name.eq_ignore_ascii_case("title"))
        })?;
        Some(descendant_text(dom, title_id))
    })
    .flatten()
    .unwrap_or_default();

    Ok(js_string_value(&title))
}

/// `<head>` is conventionally a direct child of `<html>`. Walk
/// `documentElement`'s element children for the first one whose
/// tag name is `head` (case-insensitive). Document-only utility.
fn find_head(dom: &DomTree) -> Option<NodeId> {
    let html = dom.document_element()?;
    dom.children(html).iter().copied().find(|&id| {
        dom.as_element(id)
            .is_some_and(|e| e.tag_name.eq_ignore_ascii_case("head"))
    })
}
