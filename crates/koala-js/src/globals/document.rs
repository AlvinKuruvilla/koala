//! `document` global + element wrapper — completes the Phase-2
//! DOM bridge.
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
//! # API surface (this chunk closes Phase 2)
//!
//! On `document`:
//!   - `getElementById(id)`
//!   - `querySelector(sel)`, `querySelectorAll(sel)`
//!   - `getElementsByTagName(name)`, `getElementsByClassName(names)`
//!   - `body`, `head`, `documentElement`, `title` (getters)
//!   - `createElement(tagName)`, `createTextNode(data)`
//!
//! On an Element wrapper:
//!   - `tagName`, `id`, `className` (snapshot props)
//!   - `getAttribute`, `hasAttribute`, `setAttribute`, `removeAttribute`
//!   - `parentElement`, `children`, `firstElementChild`,
//!     `lastElementChild`, `nextElementSibling`,
//!     `previousElementSibling` (live accessors)
//!   - `textContent` (live accessor: getter + setter)
//!   - `appendChild(node)`, `removeChild(node)`
//!   - `querySelector(sel)`, `querySelectorAll(sel)` (scoped to
//!     descendants)
//!
//! Text nodes returned by `createTextNode` carry just enough shape
//! (`__nodeId`, `nodeType = 3`) to be passed to `appendChild`. A
//! richer Text wrapper (`data`/`nodeValue` accessors, etc.) is a
//! Phase-2 follow-up.
//!
//! # What's still deferred (next chunk)
//!
//! - Re-layout on mutation. Scripts run after layout today, so
//!   mutations update the DOM but the rendered output reflects the
//!   pre-script state. The dirty-tracking + re-layout wiring is a
//!   self-contained follow-up.

use boa_engine::{
    Context, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction, js_string,
    object::{
        FunctionObjectBuilder, ObjectInitializer,
        builtins::{JsArray, JsFunction},
    },
    property::Attribute,
};
use koala_css::selector::{ParsedSelector, parse_selector};
use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};

use crate::dom_handle::{with_dom, with_dom_mut};

const NODE_TYPE_ELEMENT: f64 = 1.0;
const NODE_TYPE_TEXT: f64 = 3.0;

/// Register the `document` global. Called once by
/// [`crate::globals::register_globals`].
pub fn register_document(context: &mut Context) {
    // Build all accessor getters up front so the initializer chain
    // doesn't have to re-borrow the context mid-call.
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
            NativeFunction::from_copy_closure(document_query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(document_query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(document_get_elements_by_tag_name),
            js_string!("getElementsByTagName"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(document_get_elements_by_class_name),
            js_string!("getElementsByClassName"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(document_create_element),
            js_string!("createElement"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(document_create_text_node),
            js_string!("createTextNode"),
            1,
        )
        .accessor(
            js_string!("body"),
            Some(body_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("head"),
            Some(head_getter),
            None,
            accessor_attrs,
        )
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

// ============================================================================
// document.* methods
// ============================================================================

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
fn document_query_selector(
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
fn document_query_selector_all(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = parse_query_arg(args, "querySelectorAll", context)?;
    let Some(parsed) = parsed else {
        return Ok(JsArray::new(context).into());
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
fn document_get_elements_by_tag_name(
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
fn document_get_elements_by_class_name(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let raw = required_string_arg(args, 0, "getElementsByClassName", "classNames", context)?;
    let needle: Vec<String> = raw.split_whitespace().map(str::to_string).collect();
    if needle.is_empty() {
        return Ok(JsArray::new(context).into());
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
fn document_create_element(
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

/// `document.createTextNode(data)` — allocate a new Text node in
/// the DOM and return a minimal wrapper carrying its `__nodeId`
/// (for `appendChild`) and `nodeType = 3`.
///
/// [§ 4.5 Document.createTextNode](https://dom.spec.whatwg.org/#dom-document-createtextnode)
fn document_create_text_node(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let data = required_string_arg(args, 0, "createTextNode", "data", context)?;
    let new_id = with_dom_mut(|dom| dom.alloc(NodeType::Text(data))).ok_or_else(no_dom_error)?;
    make_text_object(context, new_id)
}

// ============================================================================
// document accessors
// ============================================================================

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

#[allow(clippy::unnecessary_wraps)] // signature dictated by NativeFunction callback shape
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

    Ok(JsString::from(title.as_str()).into())
}

// ============================================================================
// Element wrapper
// ============================================================================

/// Build the JS object that represents the element at `node_id`.
///
/// `tagName` / `id` / `className` are snapshot string properties
/// since the spec exposes them as live properties but mutating
/// them currently requires `setAttribute`. The other accessors are
/// real getters that re-read the DOM on every JS read, so attribute
/// edits via `setAttribute` etc. show up immediately through them.
#[allow(clippy::similar_names)] // text_content_{getter,setter} are an intentional pair
fn make_element_object(context: &mut Context, node_id: NodeId) -> JsResult<JsValue> {
    let (tag_name, id, class_name) = with_dom(|dom| {
        dom.as_element(node_id).map(|e| {
            (
                e.tag_name.to_ascii_uppercase(),
                e.id().cloned().unwrap_or_default(),
                e.attrs.get("class").cloned().unwrap_or_default(),
            )
        })
    })
    .flatten()
    .ok_or_else(no_dom_error)?;

    #[allow(clippy::cast_precision_loss)] // NodeId well below 2^53
    let node_id_value = node_id.0 as f64;

    let parent_element_getter = getter(context, parent_element_get);
    let children_getter = getter(context, children_get);
    let first_element_child_getter = getter(context, first_element_child_get);
    let last_element_child_getter = getter(context, last_element_child_get);
    let next_element_sibling_getter = getter(context, next_element_sibling_get);
    let previous_element_sibling_getter = getter(context, previous_element_sibling_get);
    let text_content_getter = getter(context, text_content_get);
    let text_content_setter = getter(context, text_content_set);

    let accessor_attrs = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("nodeType"),
            NODE_TYPE_ELEMENT,
            Attribute::READONLY,
        )
        .property(
            js_string!("tagName"),
            JsString::from(tag_name.as_str()),
            Attribute::READONLY,
        )
        .property(
            js_string!("id"),
            JsString::from(id.as_str()),
            Attribute::READONLY,
        )
        .property(
            js_string!("className"),
            JsString::from(class_name.as_str()),
            Attribute::READONLY,
        )
        .property(
            js_string!("__nodeId"),
            node_id_value,
            Attribute::READONLY,
        )
        .function(
            NativeFunction::from_copy_closure(get_attribute),
            js_string!("getAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(has_attribute),
            js_string!("hasAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(set_attribute),
            js_string!("setAttribute"),
            2,
        )
        .function(
            NativeFunction::from_copy_closure(remove_attribute),
            js_string!("removeAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(element_append_child),
            js_string!("appendChild"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(element_remove_child),
            js_string!("removeChild"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(element_query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(element_query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .accessor(
            js_string!("parentElement"),
            Some(parent_element_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("children"),
            Some(children_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("firstElementChild"),
            Some(first_element_child_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("lastElementChild"),
            Some(last_element_child_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("nextElementSibling"),
            Some(next_element_sibling_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("previousElementSibling"),
            Some(previous_element_sibling_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("textContent"),
            Some(text_content_getter),
            Some(text_content_setter),
            accessor_attrs,
        )
        .build();

    Ok(obj.into())
}

/// Minimal Text-node wrapper. Carries the `__nodeId` and a
/// `nodeType = 3` so `appendChild` (and any future type checks)
/// can identify it. Richer Text properties (`data`, `nodeValue`,
/// `length`) are deferred — the common path for testharness is
/// `el.appendChild(document.createTextNode("…"))` where the
/// returned object is just a token consumed by `appendChild`.
#[allow(clippy::unnecessary_wraps)] // mirrors make_element_object's fallible shape
fn make_text_object(context: &mut Context, node_id: NodeId) -> JsResult<JsValue> {
    #[allow(clippy::cast_precision_loss)] // NodeId well below 2^53
    let node_id_value = node_id.0 as f64;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("nodeType"),
            NODE_TYPE_TEXT,
            Attribute::READONLY,
        )
        .property(
            js_string!("__nodeId"),
            node_id_value,
            Attribute::READONLY,
        )
        .build();

    Ok(obj.into())
}

// ============================================================================
// Element attribute methods
// ============================================================================

/// `Element.getAttribute(name)` — string or `null`.
fn get_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "getAttribute", "name", context)?;

    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .and_then(|e| e.attrs.get(&name).cloned())
    })
    .flatten();

    Ok(match value {
        Some(v) => JsString::from(v.as_str()).into(),
        None => JsValue::null(),
    })
}

/// `Element.hasAttribute(name)` — true/false.
fn has_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "hasAttribute", "name", context)?;

    let present = with_dom(|dom| {
        dom.as_element(node_id)
            .is_some_and(|e| e.attrs.contains_key(&name))
    })
    .unwrap_or(false);

    Ok(JsValue::from(present))
}

/// `Element.setAttribute(name, value)`. Always overwrites.
fn set_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "setAttribute", "name", context)?;
    let value = required_string_arg(args, 1, "setAttribute", "value", context)?;

    let _ = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.insert(name, value);
        }
    });

    Ok(JsValue::undefined())
}

/// `Element.removeAttribute(name)`. No-op if absent.
fn remove_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "removeAttribute", "name", context)?;

    let _ = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.remove(&name);
        }
    });

    Ok(JsValue::undefined())
}

// ============================================================================
// Element tree mutation
// ============================================================================

/// `Element.appendChild(node)` — append `node` as the last child.
/// If `node` already has a parent, it is first removed (DOM spec
/// requirement for "adopt the node into this's node document").
///
/// [§ 4.4 Node.appendChild](https://dom.spec.whatwg.org/#dom-node-appendchild)
fn element_append_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args.first().ok_or_else(|| missing_arg("appendChild", "node"))?;
    let child_id = node_id_from_value(child_value, context)?;

    if parent_id == child_id {
        return Err(type_error("a node cannot be its own child"));
    }

    let _ = with_dom_mut(|dom| {
        if let Some(old_parent) = dom.parent(child_id) {
            dom.remove_child(old_parent, child_id);
        }
        dom.append_child(parent_id, child_id);
    });

    Ok(child_value.clone())
}

/// `Element.removeChild(node)` — detach `node` from this element.
/// Throws `NotFoundError` (a `TypeError` here since we don't yet
/// model `DOMException`) if `node`'s parent isn't this element.
///
/// [§ 4.4 Node.removeChild](https://dom.spec.whatwg.org/#dom-node-removechild)
fn element_remove_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args.first().ok_or_else(|| missing_arg("removeChild", "node"))?;
    let child_id = node_id_from_value(child_value, context)?;

    let belongs = with_dom(|dom| dom.parent(child_id) == Some(parent_id)).unwrap_or(false);
    if !belongs {
        return Err(type_error("removeChild: node is not a child of this element"));
    }

    let _ = with_dom_mut(|dom| dom.remove_child(parent_id, child_id));

    Ok(child_value.clone())
}

// ============================================================================
// Element selector methods (scoped to descendants of the receiver)
// ============================================================================

fn element_query_selector(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope_id = node_id_from_this(this, context)?;
    let parsed = parse_query_arg(args, "querySelector", context)?;
    let Some(parsed) = parsed else { return Ok(JsValue::null()) };

    let result =
        with_dom(|dom| find_first_match(dom, scope_id, &parsed)).flatten();

    match result {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn element_query_selector_all(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope_id = node_id_from_this(this, context)?;
    let parsed = parse_query_arg(args, "querySelectorAll", context)?;
    let Some(parsed) = parsed else {
        return Ok(JsArray::new(context).into());
    };

    let ids: Vec<NodeId> = with_dom(|dom| find_all_matches(dom, scope_id, &parsed))
        .unwrap_or_default();

    array_of_element_objects(ids, context)
}

// ============================================================================
// Element tree-nav accessors
// ============================================================================

fn parent_element_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let parent = with_dom(|dom| {
        dom.parent(node_id)
            .filter(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match parent {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn children_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let child_ids: Vec<NodeId> = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .filter(|&id| dom.as_element(id).is_some())
            .collect()
    })
    .unwrap_or_default();
    array_of_element_objects(child_ids, context)
}

fn first_element_child_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let first = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .find(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match first {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn last_element_child_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let last = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .rev()
            .find(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match last {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn next_element_sibling_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let next = with_dom(|dom| {
        let mut cur = dom.next_sibling(node_id);
        while let Some(id) = cur {
            if dom.as_element(id).is_some() {
                return Some(id);
            }
            cur = dom.next_sibling(id);
        }
        None
    })
    .flatten();
    match next {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

fn previous_element_sibling_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let prev = with_dom(|dom| {
        let mut cur = dom.prev_sibling(node_id);
        while let Some(id) = cur {
            if dom.as_element(id).is_some() {
                return Some(id);
            }
            cur = dom.prev_sibling(id);
        }
        None
    })
    .flatten();
    match prev {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

// ============================================================================
// textContent
// ============================================================================

/// `Element.textContent` (getter) — concatenation of every Text
/// descendant's data, in tree order.
///
/// [§ 3.4 Node.textContent](https://dom.spec.whatwg.org/#dom-node-textcontent)
fn text_content_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let text = with_dom(|dom| descendant_text(dom, node_id)).unwrap_or_default();
    Ok(JsString::from(text.as_str()).into())
}

/// `Element.textContent` (setter) — replace all children with a
/// single Text node carrying the assigned string. Empty string
/// leaves the element with no children.
///
/// [§ 3.4 Node.textContent](https://dom.spec.whatwg.org/#dom-node-textcontent)
fn text_content_set(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let new_text = args
        .first()
        .map(|v| v.to_string(context))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let _ = with_dom_mut(|dom| {
        // Snapshot then remove — we can't iterate the live slice
        // while mutating through `remove_child`.
        let children: Vec<NodeId> = dom.children(node_id).to_vec();
        for child in children {
            dom.remove_child(node_id, child);
        }
        if !new_text.is_empty() {
            let text_id = dom.alloc(NodeType::Text(new_text));
            dom.append_child(node_id, text_id);
        }
    });

    Ok(JsValue::undefined())
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Wrap a fn-pointer as a [`JsFunction`] suitable for an accessor
/// or any other place that wants a `JsFunction` rather than a
/// `NativeFunction`. fn pointers are `Copy` so the closure goes
/// through `from_copy_closure`.
fn getter(
    context: &mut Context,
    f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) -> JsFunction {
    let realm = context.realm().clone();
    FunctionObjectBuilder::new(&realm, NativeFunction::from_copy_closure(f)).build()
}

/// Read the `__nodeId` slot off the JS-side wrapper of the
/// receiver. Returns a `TypeError` if the receiver isn't an object
/// or doesn't carry a `__nodeId` number.
fn node_id_from_this(this: &JsValue, context: &mut Context) -> JsResult<NodeId> {
    let obj = this
        .as_object()
        .ok_or_else(|| type_error("method called on a non-object value"))?;
    node_id_from_object(obj, context)
}

/// Read `__nodeId` from a passed-in argument value (an Element or
/// Text wrapper). Used by [`element_append_child`] and similar
/// methods that take a node as an argument.
fn node_id_from_value(value: &JsValue, context: &mut Context) -> JsResult<NodeId> {
    let obj = value
        .as_object()
        .ok_or_else(|| type_error("expected a node, got a non-object value"))?;
    node_id_from_object(obj, context)
}

fn node_id_from_object(obj: &JsObject, context: &mut Context) -> JsResult<NodeId> {
    let raw = obj.get(js_string!("__nodeId"), context)?;
    let n = raw.to_u32(context)? as usize;
    Ok(NodeId(n))
}

/// Convert a list of element [`NodeId`]s into a JS Array of element
/// wrappers. Builds wrappers individually and pushes into a
/// [`JsArray`], so a wrapper-construction error short-circuits the
/// whole collection.
fn array_of_element_objects(
    ids: Vec<NodeId>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let mut elements = Vec::with_capacity(ids.len());
    for id in ids {
        elements.push(make_element_object(context, id)?);
    }
    Ok(JsArray::from_iter(elements, context).into())
}

/// Pull arg `index` as a string with a uniform error message.
fn required_string_arg(
    args: &[JsValue],
    index: usize,
    method: &'static str,
    arg_name: &'static str,
    context: &mut Context,
) -> JsResult<String> {
    let arg = args.get(index).ok_or_else(|| missing_arg(method, arg_name))?;
    Ok(arg.to_string(context)?.to_std_string_escaped())
}

/// Pull the selector argument out and parse it. Selector *lists*
/// (the comma-separated form `"div, p"`) are split here and parsed
/// individually, since `koala_css::parse_selector` only understands
/// a single complex selector. Returns `None` if the argument is
/// empty or every part fails to parse — per spec we'd throw
/// `SyntaxError`, but we don't yet expose `DOMException`, so we
/// surface as "no match" and let the caller pick between `null`
/// and `[]`.
fn parse_query_arg(
    args: &[JsValue],
    method: &'static str,
    context: &mut Context,
) -> JsResult<Option<Vec<ParsedSelector>>> {
    let raw = required_string_arg(args, 0, method, "selectors", context)?;
    let mut parsed = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(sel) = parse_selector(trimmed) {
            parsed.push(sel);
        }
    }
    Ok(if parsed.is_empty() { None } else { Some(parsed) })
}

/// First element under `scope` (in document order) matching any
/// selector in `parsed`. Per the DOM spec, when a selector list is
/// given, the result is the first descendant matching ANY of the
/// listed selectors — chosen by *document order*, not selector
/// order.
fn find_first_match(
    dom: &DomTree,
    scope: NodeId,
    parsed: &[ParsedSelector],
) -> Option<NodeId> {
    dom.descendants(scope).find(|&id| {
        dom.as_element(id).is_some()
            && parsed.iter().any(|p| p.matches_in_tree(dom, id))
    })
}

/// Every element under `scope` matching at least one selector in
/// `parsed`, in tree order. No deduplication needed — an element
/// is its own [`NodeId`], and we visit each one exactly once.
fn find_all_matches(
    dom: &DomTree,
    scope: NodeId,
    parsed: &[ParsedSelector],
) -> Vec<NodeId> {
    dom.descendants(scope)
        .filter(|&id| {
            dom.as_element(id).is_some()
                && parsed.iter().any(|p| p.matches_in_tree(dom, id))
        })
        .collect()
}

/// `<head>` is conventionally a direct child of `<html>`. Walk
/// `documentElement`'s element children for the first one whose
/// tag name is `head` (case-insensitive).
fn find_head(dom: &DomTree) -> Option<NodeId> {
    let html = dom.document_element()?;
    dom.children(html).iter().copied().find(|&id| {
        dom.as_element(id)
            .is_some_and(|e| e.tag_name.eq_ignore_ascii_case("head"))
    })
}

/// Concatenate all Text descendants of `node_id` in tree order.
/// Backs both `Element.textContent` getter and `document.title`.
fn descendant_text(dom: &DomTree, node_id: NodeId) -> String {
    let mut buf = String::new();
    for id in dom.descendants(node_id) {
        if let Some(s) = dom.as_text(id) {
            buf.push_str(s);
        }
    }
    buf
}

fn type_error(message: &str) -> JsError {
    JsError::from_native(JsNativeError::typ().with_message(message.to_string()))
}

fn missing_arg(method: &'static str, arg: &'static str) -> JsError {
    type_error(&format!("{method} requires the `{arg}` argument"))
}

fn no_dom_error() -> JsError {
    type_error("no DOM is currently installed for this JsRuntime")
}
