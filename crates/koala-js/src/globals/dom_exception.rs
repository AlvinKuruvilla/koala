//! `DOMException` — the spec-named error class JS throws when
//! a DOM operation fails.
//!
//! [§ 3 DOMException](https://webidl.spec.whatwg.org/#idl-DOMException)
//!
//! ```idl
//! [Exposed=*, Serializable]
//! interface DOMException {
//!   constructor(optional DOMString message = "",
//!               optional DOMString name = "Error");
//!   readonly attribute DOMString name;
//!   readonly attribute DOMString message;
//!   readonly attribute unsigned short code;
//!
//!   // Legacy code constants — preserved for compatibility.
//!   const unsigned short INDEX_SIZE_ERR = 1;
//!   const unsigned short HIERARCHY_REQUEST_ERR = 3;
//!   const unsigned short WRONG_DOCUMENT_ERR = 4;
//!   const unsigned short INVALID_CHARACTER_ERR = 5;
//!   const unsigned short NO_MODIFICATION_ALLOWED_ERR = 7;
//!   const unsigned short NOT_FOUND_ERR = 8;
//!   const unsigned short NOT_SUPPORTED_ERR = 9;
//!   const unsigned short INUSE_ATTRIBUTE_ERR = 10;
//!   const unsigned short INVALID_STATE_ERR = 11;
//!   const unsigned short SYNTAX_ERR = 12;
//!   const unsigned short INVALID_MODIFICATION_ERR = 13;
//!   const unsigned short NAMESPACE_ERR = 14;
//!   const unsigned short INVALID_ACCESS_ERR = 15;
//!   const unsigned short TYPE_MISMATCH_ERR = 17;
//!   const unsigned short SECURITY_ERR = 18;
//!   const unsigned short NETWORK_ERR = 19;
//!   const unsigned short ABORT_ERR = 20;
//!   const unsigned short URL_MISMATCH_ERR = 21;
//!   const unsigned short QUOTA_EXCEEDED_ERR = 22;
//!   const unsigned short TIMEOUT_ERR = 23;
//!   const unsigned short INVALID_NODE_TYPE_ERR = 24;
//!   const unsigned short DATA_CLONE_ERR = 25;
//! };
//! ```
//!
//! # Why this matters
//!
//! WPT's `assert_throws_dom(name, fn)` checks that `fn` throws a
//! DOMException whose `.name` matches the expected name. Today
//! every koala engine-side throw is a plain `TypeError`, which
//! fails `assert_throws_dom` (~76 subtest failures across
//! `/dom/nodes/`). This module gives engine code a way to throw
//! real `DOMException`s and gives JS a constructor it can
//! invoke (`new DOMException("msg", "InvalidStateError")`).
//!
//! # Per-instance state
//!
//! Each `DOMException` instance carries `name`, `message`, and
//! `code` in its native data (different from the
//! `__nodeId`-keyed Node wrappers — DOMException isn't bound to
//! the DOM tree). Accessors read those fields via
//! `JsObject::downcast_ref::<DomExceptionData>()`.

use std::sync::OnceLock;

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, js_string,
};
use boa_gc::{Finalize, Trace};

/// Per-instance state for a `DOMException`. The three fields
/// directly back the spec's `name` / `message` / `code` IDL
/// attributes.
#[derive(Debug, Clone, Trace, Finalize, JsData)]
pub(crate) struct DomExceptionData {
    #[unsafe_ignore_trace]
    name: String,
    #[unsafe_ignore_trace]
    message: String,
    code: u16,
}

dom_interface! {
    name: "DOMException",
    data: DomExceptionData,
    constructible: (dom_exception_construct),
    methods: [],
    accessors: [
        ("name", get(dom_exception_name_get)),
        ("message", get(dom_exception_message_get)),
        ("code", get(dom_exception_code_get)),
    ],
    register: register_dom_exception_class,
}

/// `new DOMException(message, name)` — both args are optional;
/// defaults follow the spec. Per § 3, the IDL signature is
/// `constructor(optional DOMString message = "", optional
/// DOMString name = "Error")`.
fn dom_exception_construct(
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<DomExceptionData> {
    let message = read_optional_string(args.get_or_undefined(0), "", context)?;
    let name = read_optional_string(args.get_or_undefined(1), "Error", context)?;
    let code = legacy_code_for_name(&name);
    Ok(DomExceptionData {
        name,
        message,
        code,
    })
}

/// Coerce an IDL `optional DOMString` argument to a Rust
/// `String`, applying the provided default when the argument is
/// `undefined`. `null` and other types are stringified per
/// ECMAScript ToString — matches what real browsers do for IDL
/// DOMString arguments.
fn read_optional_string(
    value: &JsValue,
    default: &str,
    context: &mut Context,
) -> JsResult<String> {
    if value.is_undefined() {
        return Ok(default.to_owned());
    }
    Ok(value.to_string(context)?.to_std_string_escaped())
}

/// Map a DOMException `name` to its legacy numeric `code` per
/// the WebIDL DOMException § 3 "Error Names" table. Names not on
/// the legacy list resolve to `0` (per the spec — `code` is a
/// legacy field that newer error names don't populate).
fn legacy_code_for_name(name: &str) -> u16 {
    match name {
        "IndexSizeError" => 1,
        "HierarchyRequestError" => 3,
        "WrongDocumentError" => 4,
        "InvalidCharacterError" => 5,
        "NoModificationAllowedError" => 7,
        "NotFoundError" => 8,
        "NotSupportedError" => 9,
        "InUseAttributeError" => 10,
        "InvalidStateError" => 11,
        "SyntaxError" => 12,
        "InvalidModificationError" => 13,
        "NamespaceError" => 14,
        "InvalidAccessError" => 15,
        "TypeMismatchError" => 17,
        "SecurityError" => 18,
        "NetworkError" => 19,
        "AbortError" => 20,
        "URLMismatchError" => 21,
        "QuotaExceededError" => 22,
        "TimeoutError" => 23,
        "InvalidNodeTypeError" => 24,
        "DataCloneError" => 25,
        _ => 0,
    }
}

fn data_from_this(this: &JsValue) -> JsResult<DomExceptionData> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("DOMException accessor called on non-object receiver"),
        )
    })?;
    let data = obj.downcast_ref::<DomExceptionData>().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ().with_message(
                "DOMException accessor called on a receiver that is not a DOMException",
            ),
        )
    })?;
    Ok(data.clone())
}

fn dom_exception_name_get(
    this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let data = data_from_this(this)?;
    Ok(JsValue::from(js_string!(data.name.as_str())))
}

fn dom_exception_message_get(
    this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let data = data_from_this(this)?;
    Ok(JsValue::from(js_string!(data.message.as_str())))
}

fn dom_exception_code_get(
    this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let data = data_from_this(this)?;
    Ok(JsValue::from(f64::from(data.code)))
}

/// Engine-side helper: build a `JsError` that JS code will see
/// as a `DOMException` with the given name + message.
///
/// We can't *construct* a JsObject without a `&mut Context`, and
/// the call sites that need to throw (e.g. `appendChild`'s
/// pre-insertion validity check) already have one in scope.
/// Returns the boxed error rather than a JsValue so the call
/// site is `return Err(throw_dom_exception(…))`.
///
/// # Panics
///
/// Panics if the `DOMException` class has not been registered.
/// Registration happens once at [`crate::JsRuntime::new`] time,
/// so any code reachable from script execution is safe.
//
// `allow(dead_code)` because no engine-side call site has been
// migrated to use this yet — it lands as infrastructure for the
// upcoming pre-insertion-validity / namespace / etc. throw
// sites. Remove the attribute when the first real call site
// (likely `Node.prototype.appendChild`) starts throwing
// HierarchyRequestError.
#[allow(dead_code)]
pub(crate) fn throw_dom_exception(
    name: &str,
    message: &str,
    context: &mut Context,
) -> JsError {
    // Construct a fresh DOMException instance and wrap it as
    // the error. Using `Class::from_data` keeps the prototype
    // chain wired up (`err instanceof DOMException`) and gives
    // the JS-side getters the right data.
    use boa_engine::class::Class;
    let data = DomExceptionData {
        name: name.to_owned(),
        message: message.to_owned(),
        code: legacy_code_for_name(name),
    };
    match DomExceptionData::from_data(data, context) {
        Ok(obj) => JsError::from_opaque(JsValue::from(obj)),
        Err(e) => {
            // If even the construction fails, fall back to a
            // plain TypeError — this means the DOMException
            // class isn't registered, which is a koala bug
            // rather than a runtime condition. Diagnostic
            // message includes the original name + message so
            // the failure mode is still debuggable.
            let _ = installed_warning();
            JsError::from_native(
                JsNativeError::typ()
                    .with_message(format!("{name}: {message} ({e})")),
            )
        }
    }
}

/// Once-only stderr warning so an under-the-hood throw failure
/// gets surfaced exactly once instead of spamming on every
/// thrown exception.
#[allow(dead_code)]
fn installed_warning() {
    static WARNED: OnceLock<()> = OnceLock::new();
    let _ = WARNED.set(());
}
