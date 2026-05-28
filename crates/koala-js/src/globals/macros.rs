//! `dom_interface!` — declarative macro for DOM-interface Class
//! registration.
//!
//! Collapses the Class-trait + register-fn boilerplate
//! validated by [`super::event_target_class`] and
//! [`super::node_class`] into a single invocation per interface.
//! One macro call expands to:
//!
//! - `impl boa_engine::class::Class for $data`, with the right
//!   `NAME`, `LENGTH`, `data_constructor`, and `init` — methods
//!   attached to the prototype, accessors attached to the
//!   prototype, all read from the caller's `methods:` /
//!   `accessors:` lists.
//! - A `pub(super) fn $register_fn(context)` that calls
//!   `context.register_global_class::<$data>()` and (when a
//!   `parent:` is given) stitches `$data.prototype.[[Prototype]]`
//!   to the parent interface's prototype.
//!
//! # Why a macro and not a function or builder
//!
//! Boa's `Class` is a *trait* — its methods are resolved
//! statically per `Self`, so we can't pull the per-interface
//! configuration into runtime data without paying for dynamic
//! dispatch (and losing the `register_global_class::<T>()`
//! single-call ergonomics). A macro emits the trait impl
//! directly, which lets the engine treat every DOM interface as
//! a "regular" Class. The pattern mirrors what Servo's IDL
//! codegen does (`bindings/codegen/codegen.py` → generated
//! `impl Methods for Element { … }` blocks) without committing
//! to a full Web-IDL parser.
//!
//! # Syntax
//!
//! ```text
//! dom_interface! {
//!     name: "<JS-side interface name>",
//!     data: <Rust data type, must impl Trace + Finalize + JsData>,
//!     parent: "<parent interface name>",   // optional
//!     constructible: <true | false>,
//!     methods: [
//!         ("<JS name>", <arity>, <Rust fn path>),
//!         …
//!     ],
//!     accessors: [
//!         ("<JS name>", get(<Rust fn path>)),                      // read-only
//!         ("<JS name>", get(<Rust fn path>), set(<Rust fn path>)), // read-write
//!         …
//!     ],
//!     register: <Rust fn name for the register entry-point>,
//! }
//! ```
//!
//! Three constructor modes are accepted via the
//! `constructible:` field:
//!
//! - `constructible: false` — abstract / illegal. The generated
//!   `data_constructor` throws `TypeError: Illegal constructor:
//!   <name> is not constructible`. The data type can be a
//!   zero-sized marker.
//! - `constructible: true` — constructible with no inspection
//!   of arguments. The data type must define
//!   `impl $data { fn new() -> Self { … } }`; the macro's
//!   `data_constructor` body is `Ok(Self::new())`.
//! - `constructible: (some_fn_path)` — constructible with args.
//!   The parens wrap the function path so it arrives at the
//!   internal `@class` dispatch as a single token tree. The
//!   user function has signature
//!   `fn(args: &[JsValue], context: &mut Context) ->
//!   JsResult<Self>` and owns both arg-coercion and any
//!   spec-defined throwing (e.g. `DOMException`'s
//!   `optional DOMString message = ""`).
//!
//! # Tuple syntax for `methods:` and `accessors:`
//!
//! Each `methods` entry is `("name", arity, fn_path)` and each
//! `accessors` entry is `("name", get(fn))` or
//! `("name", get(fn), set(fn))`. The tuple parens are
//! load-bearing — `macro_rules!` only allows a small set of
//! follow-tokens after a `path` fragment, and the closing `)`
//! is one of them. List form (square brackets, comma-separated)
//! lets the macro recur cleanly with `$( … ),*`.
//!
//! # Two scopes for method/accessor function bodies
//!
//! The methods and accessors named in the macro must be plain
//! Rust functions with the standard NativeFunction signature:
//!
//! ```ignore
//! fn(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue>
//! ```
//!
//! How they recover instance state is up to the caller:
//!
//! - For interfaces backed by a real Rust data type
//!   (`EventTargetData`), use
//!   `this.as_object()?.downcast_ref::<DataType>()`.
//! - For interfaces whose wrappers carry only a `__nodeId` JS
//!   property (everything reachable from `Node` today), use
//!   `super::helpers::node_id_from_this(this, context)`.
//!
//! The macro doesn't dictate; it just installs the function
//! references on the prototype.

/// Declarative DOM-interface registration. See the module-level
/// docs for syntax and rationale.
#[macro_export]
macro_rules! dom_interface {
    // Variant with a parent interface (inheritance link).
    (
        $(#[$meta:meta])*
        name: $name:literal,
        data: $data:ty,
        parent: $parent:literal,
        constructible: $constructible:tt,
        methods: [ $( ( $mname:literal, $marity:literal, $mfn:path ) ),* $(,)? ],
        accessors: [
            $( ( $aname:literal, get( $aget:path ) $(, set( $aset:path ) )? ) ),* $(,)?
        ],
        register: $register_fn:ident $(,)?
    ) => {
        $crate::__dom_interface_impl!(@class $data, $name, $constructible);
        $crate::__dom_interface_impl!(
            @init $data,
            methods: [ $( ( $mname, $marity, $mfn ) ),* ],
            accessors: [
                $( ( $aname, get( $aget ) $(, set( $aset ) )? ) ),*
            ]
        );

        $(#[$meta])*
        pub(super) fn $register_fn(context: &mut ::boa_engine::Context) {
            context
                .register_global_class::<$data>()
                .unwrap_or_else(|_| panic!(
                    "{} class should not already be registered",
                    $name,
                ));
            $crate::__dom_interface_impl!(
                @stitch_parent context, $name, $parent
            );
        }
    };

    // Variant with no parent — top of the chain, currently just EventTarget.
    (
        $(#[$meta:meta])*
        name: $name:literal,
        data: $data:ty,
        constructible: $constructible:tt,
        methods: [ $( ( $mname:literal, $marity:literal, $mfn:path ) ),* $(,)? ],
        accessors: [
            $( ( $aname:literal, get( $aget:path ) $(, set( $aset:path ) )? ) ),* $(,)?
        ],
        register: $register_fn:ident $(,)?
    ) => {
        $crate::__dom_interface_impl!(@class $data, $name, $constructible);
        $crate::__dom_interface_impl!(
            @init $data,
            methods: [ $( ( $mname, $marity, $mfn ) ),* ],
            accessors: [
                $( ( $aname, get( $aget ) $(, set( $aset ) )? ) ),*
            ]
        );

        $(#[$meta])*
        pub(super) fn $register_fn(context: &mut ::boa_engine::Context) {
            context
                .register_global_class::<$data>()
                .unwrap_or_else(|_| panic!(
                    "{} class should not already be registered",
                    $name,
                ));
        }
    };
}

/// Internal helper macro: emits the Class trait impl with the
/// right `data_constructor` body for `illegal` vs constructible,
/// and the `init` body that calls `class.method` / `class.accessor`
/// for each entry. Split out so the top-level `dom_interface!`
/// arms stay readable.
///
/// Has to be `#[macro_export]` because the top-level
/// `dom_interface!` calls into it via `$crate::...`. The leading
/// double-underscore marks it as internal — direct callers
/// should always use `dom_interface!`.
#[macro_export]
#[doc(hidden)]
macro_rules! __dom_interface_impl {
    // Class impl with abstract / illegal constructor.
    (@class $data:ty, $name:literal, false) => {
        impl ::boa_engine::class::Class for $data {
            const NAME: &'static str = $name;
            const LENGTH: usize = 0;

            fn data_constructor(
                _new_target: &::boa_engine::JsValue,
                _args: &[::boa_engine::JsValue],
                _context: &mut ::boa_engine::Context,
            ) -> ::boa_engine::JsResult<Self> {
                Err(::boa_engine::JsError::from_native(
                    ::boa_engine::JsNativeError::typ().with_message(concat!(
                        "Illegal constructor: ",
                        $name,
                        " is not constructible",
                    )),
                ))
            }

            fn init(
                class: &mut ::boa_engine::class::ClassBuilder<'_>,
            ) -> ::boa_engine::JsResult<()> {
                <$data>::__dom_interface_init(class)
            }
        }
    };

    // Class impl with constructible (no-args) constructor.
    // Calls `<$data>::new()`. The user must provide `impl $data
    // { fn new() -> Self }`.
    (@class $data:ty, $name:literal, true) => {
        impl ::boa_engine::class::Class for $data {
            const NAME: &'static str = $name;
            const LENGTH: usize = 0;

            fn data_constructor(
                _new_target: &::boa_engine::JsValue,
                _args: &[::boa_engine::JsValue],
                _context: &mut ::boa_engine::Context,
            ) -> ::boa_engine::JsResult<Self> {
                Ok(<$data>::new())
            }

            fn init(
                class: &mut ::boa_engine::class::ClassBuilder<'_>,
            ) -> ::boa_engine::JsResult<()> {
                <$data>::__dom_interface_init(class)
            }
        }
    };

    // Class impl with constructible-with-args constructor.
    // The user provides a free function with signature
    //     fn(args: &[JsValue], context: &mut Context) -> JsResult<$data>
    // and passes its path in parens:  `constructible: (my_fn)`.
    // The parens are syntactic glue — they wrap the path so it
    // arrives at this `@class` arm as a single token tree, which
    // is the only shape `$tt`-based dispatch can route on.
    //
    // The constructor body forwards both `args` and `context` to
    // the user function, so engine-side helpers like
    // `koala_js::globals::dom_exception::dom_exception_construct`
    // can do their own arg coercion and error throwing without
    // the macro having to know IDL types.
    (@class $data:ty, $name:literal, ( $ctor_fn:path )) => {
        impl ::boa_engine::class::Class for $data {
            const NAME: &'static str = $name;
            const LENGTH: usize = 0;

            fn data_constructor(
                _new_target: &::boa_engine::JsValue,
                args: &[::boa_engine::JsValue],
                context: &mut ::boa_engine::Context,
            ) -> ::boa_engine::JsResult<Self> {
                $ctor_fn(args, context)
            }

            fn init(
                class: &mut ::boa_engine::class::ClassBuilder<'_>,
            ) -> ::boa_engine::JsResult<()> {
                <$data>::__dom_interface_init(class)
            }
        }
    };

    // Inherent impl carrying the init body.
    // The init body is hung off the data type itself (rather
    // than living inside the trait impl) so that the `methods`
    // and `accessors` lists can be expanded into straightforward
    // sequential statements without macro-level recursion. Each
    // method becomes a single `class.method(…)` call; each
    // accessor becomes `getter()` + `class.accessor(…)`. The
    // trait impl just delegates here.
    (
        @init $data:ty,
        methods: [ $( ( $mname:literal, $marity:literal, $mfn:path ) ),* ],
        accessors: [
            $( ( $aname:literal, get( $aget:path ) $(, set( $aset:path ) )? ) ),*
        ]
    ) => {
        impl $data {
            #[doc(hidden)]
            // `class` is unused when both `methods` and `accessors`
            // lists are empty (e.g. HTMLElement, which inherits its
            // entire surface from Element). The allow keeps the
            // empty-interface call site lint-clean without forcing
            // every caller to add at least one method.
            #[allow(unused_variables)]
            fn __dom_interface_init(
                class: &mut ::boa_engine::class::ClassBuilder<'_>,
            ) -> ::boa_engine::JsResult<()> {
                $(
                    let _ = class.method(
                        ::boa_engine::js_string!($mname),
                        $marity,
                        ::boa_engine::NativeFunction::from_fn_ptr($mfn),
                    );
                )*

                #[allow(unused_variables)] // empty `accessors:` block
                let attrs = ::boa_engine::property::Attribute::CONFIGURABLE
                    | ::boa_engine::property::Attribute::ENUMERABLE;
                $(
                    let getter = $crate::globals::helpers::getter(
                        class.context(),
                        $aget,
                    );
                    #[allow(unused_mut, unused_assignments)]
                    let mut setter: Option<::boa_engine::object::builtins::JsFunction> = None;
                    $(
                        setter = Some($crate::globals::helpers::getter(
                            class.context(),
                            $aset,
                        ));
                    )?
                    let _ = class.accessor(
                        ::boa_engine::js_string!($aname),
                        Some(getter),
                        setter,
                        attrs,
                    );
                )*

                Ok(())
            }
        }
    };

    // Post-registration prototype-chain stitch.
    // After `register_global_class::<$data>()` runs, walk through
    // the global object to find this class's prototype and the
    // parent class's prototype, then set the former's
    // `[[Prototype]]` to the latter. This matches what
    // `node_class::register_node_class` did by hand for the
    // EventTarget link, just parameterised.
    (@stitch_parent $context:ident, $name:literal, $parent:literal) => {
        {
            let lookup_proto = |ctor_name: &str, cx: &mut ::boa_engine::Context|
                -> Option<::boa_engine::JsObject>
            {
                let global = cx.global_object();
                let ctor = global.get(
                    ::boa_engine::JsString::from(ctor_name),
                    cx,
                ).ok()?;
                let ctor_obj = ctor.as_object()?;
                let proto = ctor_obj.get(
                    ::boa_engine::js_string!("prototype"),
                    cx,
                ).ok()?;
                proto.as_object()
            };
            let child_proto = lookup_proto($name, $context)
                .unwrap_or_else(|| panic!(
                    "{}.prototype must be readable post-registration",
                    $name,
                ));
            let parent_proto = lookup_proto($parent, $context)
                .unwrap_or_else(|| panic!(
                    "parent {}.prototype must be registered before {}",
                    $parent,
                    $name,
                ));
            let _ = child_proto.set_prototype(Some(parent_proto));
        }
    };
}
