//! Console API implementation.
//!
//! [Console Standard](https://console.spec.whatwg.org/)
//!
//! This module implements the `console` global object with `log`, `warn`,
//! and `error` methods that output to stdout/stderr.

use boa_engine::{
    Context, JsResult, JsValue, NativeFunction, js_string, object::ObjectInitializer,
    property::Attribute,
};

/// Register the console global object on the context.
///
/// [§ 1.1 Logging](https://console.spec.whatwg.org/#logging)
///
/// Creates a `console` object with the following methods:
/// - `console.log(...args)` - Logs to stdout
/// - `console.warn(...args)` - Logs to stdout with warning prefix
/// - `console.error(...args)` - Logs to stderr
///
/// # Not Yet Implemented
///
/// The following Console Standard methods are not yet implemented:
///
/// [§ 1.1.1 debug](https://console.spec.whatwg.org/#debug)
/// "Perform Logger("debug", data)."
///
/// [§ 1.1.1 info](https://console.spec.whatwg.org/#info)
/// "Perform Logger("info", data)."
///
/// [§ 1.1.4 assert](https://console.spec.whatwg.org/#assert)
/// "If condition is false, perform Logger("assert", data)."
///
/// [§ 1.2 Counting](https://console.spec.whatwg.org/#counting)
/// "count(label)" and "countReset(label)" for counting labeled calls.
///
/// [§ 1.3 Grouping](https://console.spec.whatwg.org/#grouping)
/// `group()`, `groupCollapsed()`, `groupEnd()` for nested logging.
///
/// [§ 1.4 Timing](https://console.spec.whatwg.org/#timing)
/// `time(label)`, `timeLog(label)`, `timeEnd(label)` for performance timing.
///
/// [§ 1.5 Table](https://console.spec.whatwg.org/#table)
/// `table(tabularData)` for tabular data display.
///
/// [§ 1.6 Trace](https://console.spec.whatwg.org/#trace)
/// `trace()` for stack trace output.
///
/// [§ 1.7 Clear](https://console.spec.whatwg.org/#clear)
/// `clear()` to clear the console.
pub fn register_console(context: &mut Context) {
    let console = ObjectInitializer::new(context)
        .function(NativeFunction::from_copy_closure(console_log), js_string!("log"), 0)
        .function(NativeFunction::from_copy_closure(console_warn), js_string!("warn"), 0)
        .function(NativeFunction::from_copy_closure(console_error), js_string!("error"), 0)
        // TODO: Implement remaining console methods per Console Standard
        // .function(NativeFunction::from_copy_closure(console_debug), js_string!("debug"), 0)
        // .function(NativeFunction::from_copy_closure(console_info), js_string!("info"), 0)
        // .function(NativeFunction::from_copy_closure(console_assert), js_string!("assert"), 0)
        // .function(NativeFunction::from_copy_closure(console_trace), js_string!("trace"), 0)
        // .function(NativeFunction::from_copy_closure(console_clear), js_string!("clear"), 0)
        .build();

    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .expect("console global should not already exist");
}

/// `console.log(...args)` - Logs arguments to stdout.
///
/// [§ 1.1.1 log](https://console.spec.whatwg.org/#log)
///
/// "Perform Logger("log", data)."
fn console_log(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let output = format_console_args(args, context)?;
    println!("[JS] {output}");
    Ok(JsValue::undefined())
}

/// `console.warn(...args)` - Logs arguments to stdout with warning prefix.
///
/// [§ 1.1.3 warn](https://console.spec.whatwg.org/#warn)
///
/// "Perform Logger("warn", data)."
fn console_warn(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let output = format_console_args(args, context)?;
    println!("[JS WARN] {output}");
    Ok(JsValue::undefined())
}

/// `console.error(...args)` - Logs arguments to stderr.
///
/// [§ 1.1.2 error](https://console.spec.whatwg.org/#error)
///
/// "Perform Logger("error", data)."
fn console_error(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let output = format_console_args(args, context)?;
    eprintln!("[JS ERROR] {output}");
    Ok(JsValue::undefined())
}

/// Format console arguments for output.
///
/// [§ 2.1 Formatter](https://console.spec.whatwg.org/#formatter)
///
/// Converts each argument to a string and joins them with spaces.
fn format_console_args(args: &[JsValue], context: &mut Context) -> JsResult<String> {
    let strings: Result<Vec<String>, _> = args
        .iter()
        .map(|arg| arg.to_string(context).map(|s| s.to_std_string_escaped()))
        .collect();

    Ok(strings?.join(" "))
}
