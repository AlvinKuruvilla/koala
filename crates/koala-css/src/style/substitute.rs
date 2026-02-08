//! CSS Custom Properties `var()` Substitution
//!
//! [CSS Custom Properties for Cascading Variables Module Level 1 § 3](https://www.w3.org/TR/css-variables-1/#using-variables)
//!
//! "If a property value contains one or more `var()` functions, and those
//! functions are syntactically valid, the entire property's grammar must be
//! assumed to be valid at parse time. It is only syntax-checked at
//! computed-value time, after `var()` functions have been substituted."

use std::collections::HashMap;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

/// Maximum substitution depth to prevent infinite recursion from cycles.
///
/// [§ 2.3 Resolving Dependency Cycles](https://www.w3.org/TR/css-variables-1/#cycles)
///
/// "If there is a cycle in the dependency graph, all the custom properties
/// in the cycle are invalid at computed-value time."
///
/// We use a depth limit as a pragmatic approximation of cycle detection.
const MAX_SUBSTITUTION_DEPTH: u32 = 32;

/// [§ 3 Using Cascading Variables](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// Check if component values contain any `var()` function references.
#[must_use]
pub fn contains_var(values: &[ComponentValue]) -> bool {
    for cv in values {
        match cv {
            ComponentValue::Function { name, value } => {
                if name.eq_ignore_ascii_case("var") {
                    return true;
                }
                // Check nested functions too (e.g. calc(var(--x) + 1))
                if contains_var(value) {
                    return true;
                }
            }
            ComponentValue::Block { value, .. } => {
                if contains_var(value) {
                    return true;
                }
            }
            ComponentValue::Token(_) => {}
        }
    }
    false
}

/// [§ 3 Using Cascading Variables](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// "To substitute a `var()` in a property's value:
///  1. If the custom property named by the first argument to the `var()`
///     function is animation-tainted, and the `var()` function is being used
///     in the animation property or one of its longhands, treat the custom
///     property as having its initial value for the rest of this algorithm.
///  2. If the value of the custom property named by the first argument to
///     the `var()` function is anything but the initial value, replace the
///     `var()` function by the value of the corresponding custom property.
///  3. Otherwise, if the `var()` function has a fallback value as its second
///     argument, replace the `var()` function by the fallback value. If there
///     are any `var()` references in the fallback, substitute them as well.
///  4. Otherwise, the property containing the `var()` function is invalid at
///     computed-value time."
///
/// Returns `None` if substitution fails (guaranteed-invalid / cycle).
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn substitute_var(
    values: &[ComponentValue],
    custom_properties: &HashMap<String, Vec<ComponentValue>>,
    depth: u32,
) -> Option<Vec<ComponentValue>> {
    // [§ 2.3](https://www.w3.org/TR/css-variables-1/#cycles)
    // Depth limit as cycle detection approximation.
    if depth > MAX_SUBSTITUTION_DEPTH {
        return None;
    }

    let mut result = Vec::with_capacity(values.len());

    for cv in values {
        match cv {
            ComponentValue::Function { name, value } if name.eq_ignore_ascii_case("var") => {
                // Parse the var() function arguments.
                //
                // [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
                // "The var() function can not be used as ... It can only be used
                // as a value ... its syntax is:
                //   var() = var( <custom-property-name> [, <declaration-value>]? )"
                //
                // The first argument is the custom property name (an ident starting
                // with `--`). Everything after the first comma is the fallback value.
                let (prop_name, fallback) = parse_var_arguments(value);

                let prop_name = prop_name?;

                if let Some(prop_value) = custom_properties.get(&prop_name) {
                    // Step 2: Custom property exists — substitute its value.
                    // The value is already resolved (var() substituted) at this
                    // point for custom-property-to-custom-property references.
                    result.extend(prop_value.iter().cloned());
                } else if let Some(fb) = fallback {
                    // Step 3: Use fallback value, substituting any var() in it.
                    let resolved_fallback =
                        substitute_var(&fb, custom_properties, depth + 1)?;
                    result.extend(resolved_fallback);
                } else {
                    // Step 4: No value, no fallback — invalid at computed-value time.
                    return None;
                }
            }
            ComponentValue::Function { name, value } => {
                // Non-var() function — recursively substitute var() in its children.
                let resolved_children = substitute_var(value, custom_properties, depth + 1)?;
                result.push(ComponentValue::Function {
                    name: name.clone(),
                    value: resolved_children,
                });
            }
            ComponentValue::Block { token, value } => {
                // Recursively substitute var() in block children.
                let resolved_children = substitute_var(value, custom_properties, depth + 1)?;
                result.push(ComponentValue::Block {
                    token: *token,
                    value: resolved_children,
                });
            }
            other @ ComponentValue::Token(_) => {
                // Preserved token — pass through unchanged.
                result.push(other.clone());
            }
        }
    }

    Some(result)
}

/// Parse the arguments of a `var()` function.
///
/// [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// "`var()` = var( <custom-property-name> \[, <declaration-value>\]? )"
///
/// Returns `(property_name, fallback)`:
/// - `property_name`: The `--*` custom property name, or `None` if missing/invalid.
/// - `fallback`: Everything after the first comma, or `None` if no comma.
///
/// "var(--foo, red, blue) defines a fallback of `red, blue`."
/// Everything after the first comma (including additional commas) is the fallback.
fn parse_var_arguments(
    args: &[ComponentValue],
) -> (Option<String>, Option<Vec<ComponentValue>>) {
    // Find the custom property name (first ident starting with `--`).
    let mut prop_name: Option<String> = None;
    let mut comma_idx: Option<usize> = None;

    for (i, cv) in args.iter().enumerate() {
        match cv {
            ComponentValue::Token(CSSToken::Ident(ident)) if ident.starts_with("--") => {
                if prop_name.is_none() {
                    prop_name = Some(ident.clone());
                }
            }
            ComponentValue::Token(CSSToken::Comma) => {
                comma_idx = Some(i);
                break;
            }
            // Skip whitespace
            ComponentValue::Token(CSSToken::Whitespace) => {}
            _ => {
                // If we haven't found the property name yet, this is invalid
                if prop_name.is_none() {
                    return (None, None);
                }
            }
        }
    }

    // Build fallback from everything after the first comma.
    let fallback = comma_idx.map(|ci| {
        let fb: Vec<ComponentValue> = args[ci + 1..].to_vec();
        // Trim leading whitespace from the fallback.
        let start = fb
            .iter()
            .position(|cv| !matches!(cv, ComponentValue::Token(CSSToken::Whitespace)))
            .unwrap_or(fb.len());
        fb[start..].to_vec()
    });

    (prop_name, fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ComponentValue;
    use crate::tokenizer::CSSToken;

    /// Helper: make an Ident token component value.
    fn ident(s: &str) -> ComponentValue {
        ComponentValue::Token(CSSToken::Ident(s.to_string()))
    }

    /// Helper: make a Comma token component value.
    fn comma() -> ComponentValue {
        ComponentValue::Token(CSSToken::Comma)
    }

    /// Helper: make a var() function component value.
    fn var_fn(args: Vec<ComponentValue>) -> ComponentValue {
        ComponentValue::Function {
            name: "var".to_string(),
            value: args,
        }
    }

    /// Helper: make a whitespace token.
    fn ws() -> ComponentValue {
        ComponentValue::Token(CSSToken::Whitespace)
    }

    #[test]
    fn test_contains_var_basic() {
        // var(--color)
        let values = vec![var_fn(vec![ident("--color")])];
        assert!(contains_var(&values));
    }

    #[test]
    fn test_contains_var_none() {
        let values = vec![ident("red")];
        assert!(!contains_var(&values));
    }

    #[test]
    fn test_contains_var_nested() {
        // calc(var(--x) + 1)
        let values = vec![ComponentValue::Function {
            name: "calc".to_string(),
            value: vec![var_fn(vec![ident("--x")])],
        }];
        assert!(contains_var(&values));
    }

    #[test]
    fn test_substitute_basic() {
        // var(--color) with --color: red
        let values = vec![var_fn(vec![ident("--color")])];
        let mut props = HashMap::new();
        let _ = props.insert("--color".to_string(), vec![ident("red")]);

        let result = substitute_var(&values, &props, 0);
        assert_eq!(result, Some(vec![ident("red")]));
    }

    #[test]
    fn test_substitute_fallback() {
        // var(--missing, blue) → blue
        let values = vec![var_fn(vec![ident("--missing"), comma(), ws(), ident("blue")])];
        let props = HashMap::new();

        let result = substitute_var(&values, &props, 0);
        assert_eq!(result, Some(vec![ident("blue")]));
    }

    #[test]
    fn test_substitute_nested_fallback() {
        // var(--missing, var(--color)) with --color: green
        let values = vec![var_fn(vec![
            ident("--missing"),
            comma(),
            ws(),
            var_fn(vec![ident("--color")]),
        ])];
        let mut props = HashMap::new();
        let _ = props.insert("--color".to_string(), vec![ident("green")]);

        let result = substitute_var(&values, &props, 0);
        assert_eq!(result, Some(vec![ident("green")]));
    }

    #[test]
    fn test_substitute_comma_in_fallback() {
        // var(--missing, Arial, sans-serif) → Arial, sans-serif
        let values = vec![var_fn(vec![
            ident("--missing"),
            comma(),
            ws(),
            ident("Arial"),
            comma(),
            ws(),
            ident("sans-serif"),
        ])];
        let props = HashMap::new();

        let result = substitute_var(&values, &props, 0);
        assert_eq!(
            result,
            Some(vec![ident("Arial"), comma(), ws(), ident("sans-serif")])
        );
    }

    #[test]
    fn test_substitute_missing_no_fallback() {
        // var(--missing) → None (invalid at computed-value time)
        let values = vec![var_fn(vec![ident("--missing")])];
        let props = HashMap::new();

        let result = substitute_var(&values, &props, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_substitute_depth_limit() {
        // Depth > MAX_SUBSTITUTION_DEPTH → None
        let values = vec![var_fn(vec![ident("--a")])];
        let mut props = HashMap::new();
        let _ = props.insert("--a".to_string(), vec![ident("ok")]);

        let result = substitute_var(&values, &props, MAX_SUBSTITUTION_DEPTH + 1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_substitute_no_var_passthrough() {
        // No var() present → passthrough unchanged
        let values = vec![ident("red")];
        let props = HashMap::new();

        let result = substitute_var(&values, &props, 0);
        assert_eq!(result, Some(vec![ident("red")]));
    }

    #[test]
    fn test_substitute_in_non_var_function() {
        // rgb(var(--r), 0, 0) with --r: 255
        let values = vec![ComponentValue::Function {
            name: "rgb".to_string(),
            value: vec![
                var_fn(vec![ident("--r")]),
                comma(),
                ws(),
                ComponentValue::Token(CSSToken::Number {
                    value: 0.0,
                    int_value: Some(0),
                    numeric_type: crate::tokenizer::NumericType::Integer,
                }),
                comma(),
                ws(),
                ComponentValue::Token(CSSToken::Number {
                    value: 0.0,
                    int_value: Some(0),
                    numeric_type: crate::tokenizer::NumericType::Integer,
                }),
            ],
        }];
        let mut props = HashMap::new();
        let _ = props.insert(
            "--r".to_string(),
            vec![ComponentValue::Token(CSSToken::Number {
                value: 255.0,
                int_value: Some(255),
                numeric_type: crate::tokenizer::NumericType::Integer,
            })],
        );

        let result = substitute_var(&values, &props, 0).unwrap();
        // Should be rgb(255, 0, 0)
        if let ComponentValue::Function { name, value } = &result[0] {
            assert_eq!(name, "rgb");
            if let ComponentValue::Token(CSSToken::Number { value: v, .. }) = &value[0] {
                assert_eq!(*v, 255.0);
            } else {
                panic!("Expected Number token");
            }
        } else {
            panic!("Expected Function");
        }
    }
}
