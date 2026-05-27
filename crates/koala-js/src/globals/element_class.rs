//! `Element` and `HTMLElement` as Boa-native [`Class`]es.
//!
//! [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
//! [§ 4 Interface HTMLElement](https://html.spec.whatwg.org/multipage/dom.html#htmlelement)
//!
//! Third migration in the Tier-1 DOM-binding work, and the
//! first to use the [`dom_interface!`](crate::dom_interface)
//! macro for both halves of an inheritance edge in one file.
//! Replaces the hand-rolled stubs that previously lived in
//! [`super::interfaces`].
//!
//! # Methods + accessors that move onto the prototype
//!
//! Every method and accessor previously installed as an
//! **own-property** on each Element wrapper by
//! [`super::element::make_element_object`] is now installed
//! exactly once on `Element.prototype` and inherited by every
//! wrapper via the prototype chain. The function bodies stay in
//! [`super::element`] — only the attachment point moves. This
//! brings koala-js in line with what every production browser
//! exposes: `Element.prototype.appendChild.call(other, child)`
//! works, prototype-walker assertions pass, and adding a new
//! method costs one row in the macro list instead of an entry
//! in the wrapper-builder chain.
//!
//! # Per-instance state stays on `__nodeId`
//!
//! `ElementData` / `HTMLElementData` are zero-sized markers —
//! Boa's `Class` trait requires a `Self` for the data slot, but
//! every real wrapper still carries its identity in the
//! `__nodeId` JS property that
//! [`super::element::make_element_object`] sets. Prototype
//! methods recover the [`NodeId`] via
//! [`super::helpers::node_id_from_this`]. Switching to a real
//! native-data slot would be a bigger restructure for no
//! behavioural win.
//!
//! [`NodeId`]: koala_dom::NodeId
//! [`Class`]: boa_engine::class::Class

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};

/// Zero-sized marker for `Element` Class registration. See the
/// module-level docs for why the per-wrapper data slot stays on
/// the JS `__nodeId` property instead of moving into native
/// data.
#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct ElementData;

/// Zero-sized marker for `HTMLElement`. Identical to
/// [`ElementData`] in structure — kept distinct so the two
/// classes register under different `NAME`s and end up with
/// independent prototype objects.
#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct HtmlElementData;

dom_interface! {
    name: "Element",
    data: ElementData,
    parent: "Node",
    constructible: false,
    methods: [
        ("getAttribute", 1, super::element::get_attribute),
        ("hasAttribute", 1, super::element::has_attribute),
        ("setAttribute", 2, super::element::set_attribute),
        ("removeAttribute", 1, super::element::remove_attribute),
        ("querySelector", 1, super::element::query_selector),
        ("querySelectorAll", 1, super::element::query_selector_all),
        // EventTarget mixin — element-scoped versions live here
        // rather than relying on EventTarget.prototype.* because
        // the EventTarget Class methods read scope from
        // `downcast_ref::<EventTargetData>(this)`, which fails on
        // element wrappers (no EventTargetData attached). The
        // element-scoped versions read `__nodeId` instead.
        ("addEventListener", 2, super::element::element_add_event_listener),
        ("removeEventListener", 2, super::element::element_remove_event_listener),
        ("dispatchEvent", 1, super::element::element_dispatch_event),
    ],
    accessors: [
        ("tagName", get(super::element::tag_name_get)),
        ("id", get(super::element::id_get), set(super::element::id_set)),
        ("className", get(super::element::class_name_get), set(super::element::class_name_set)),
        ("childElementCount", get(super::element::child_element_count_get)),
        ("parentElement", get(super::element::parent_element_get)),
        ("children", get(super::element::children_get)),
        ("firstElementChild", get(super::element::first_element_child_get)),
        ("lastElementChild", get(super::element::last_element_child_get)),
        ("nextElementSibling", get(super::element::next_element_sibling_get)),
        ("previousElementSibling", get(super::element::previous_element_sibling_get)),
        ("textContent", get(super::element::text_content_get), set(super::element::text_content_set)),
    ],
    register: register_element_class,
}

dom_interface! {
    name: "HTMLElement",
    data: HtmlElementData,
    parent: "Element",
    constructible: false,
    methods: [],
    accessors: [],
    register: register_html_element_class,
}
