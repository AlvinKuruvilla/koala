//! CSS Parser per [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing).
//!
//! "The input to the parsing stage is a stream of tokens from the tokenization stage."
//! This is a basic implementation that parses style rules.

use crate::tokenizer::CSSToken;

/// [§ 5.4.4 Consume a declaration](https://www.w3.org/TR/css-syntax-3/#consume-a-declaration)
///
/// A CSS declaration (e.g., `color: red`).
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    /// The property name.
    pub name: String,
    /// The property value as component values.
    pub value: Vec<ComponentValue>,
    /// Whether the declaration has `!important`.
    pub important: bool,
}

/// [§ 5.3.7 Consume a component value](https://www.w3.org/TR/css-syntax-3/#consume-a-component-value)
///
/// A component value in a declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// A preserved token.
    Token(CSSToken),
    /// A function with its contents.
    Function {
        /// The function name.
        name: String,
        /// The function arguments.
        value: Vec<ComponentValue>,
    },
    /// A simple block.
    Block {
        /// The opening token character.
        token: char,
        /// The block contents.
        value: Vec<ComponentValue>,
    },
}

/// A CSS selector (simplified representation)
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    /// Raw selector text
    pub text: String,
}

/// [§ 5.4.3 Consume a qualified rule](https://www.w3.org/TR/css-syntax-3/#consume-a-qualified-rule)
///
/// A CSS style rule (selector + declarations).
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
    /// The list of selectors for this rule.
    pub selectors: Vec<Selector>,
    /// The declarations in this rule block.
    pub declarations: Vec<Declaration>,
}

/// [§ 5.4.2 Consume an at-rule](https://www.w3.org/TR/css-syntax-3/#consume-an-at-rule)
///
/// A CSS at-rule.
#[derive(Debug, Clone, PartialEq)]
pub struct AtRule {
    /// The at-keyword name (without the `@`).
    pub name: String,
    /// The prelude component values.
    pub prelude: Vec<ComponentValue>,
    /// The optional block contents.
    pub block: Option<Vec<ComponentValue>>,
}

/// [§ 5.3.3 Consume a list of rules](https://www.w3.org/TR/css-syntax-3/#consume-list-of-rules)
///
/// A CSS rule (either a style rule or an at-rule).
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    /// A style rule (qualified rule).
    Style(StyleRule),
    /// An at-rule.
    At(AtRule),
}

/// [§ 5.3.2 Parse a stylesheet](https://www.w3.org/TR/css-syntax-3/#parse-stylesheet)
///
/// A parsed CSS stylesheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Stylesheet {
    /// The list of rules in the stylesheet.
    pub rules: Vec<Rule>,
}

/// CSS parser
pub struct CSSParser {
    tokens: Vec<CSSToken>,
    position: usize,
}

impl CSSParser {
    /// Create a new parser from a list of tokens.
    pub fn new(tokens: Vec<CSSToken>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    /// [§ 5.3.3 Parse a stylesheet](https://www.w3.org/TR/css-syntax-3/#parse-stylesheet)
    ///
    /// "To parse a stylesheet from input..."
    pub fn parse_stylesheet(&mut self) -> Stylesheet {
        // "Consume a list of rules from input, with the top-level flag set."
        let rules = self.consume_list_of_rules(true);
        Stylesheet { rules }
    }

    /// [§ 5.3.6 Parse a list of declarations](https://www.w3.org/TR/css-syntax-3/#parse-list-of-declarations)
    ///
    /// Parse declarations from a style attribute or similar.
    pub fn parse_declaration_list(&mut self) -> Vec<Declaration> {
        self.consume_list_of_declarations()
    }

    /// [§ 5.4.1 Consume a list of rules](https://www.w3.org/TR/css-syntax-3/#consume-list-of-rules)
    fn consume_list_of_rules(&mut self, top_level: bool) -> Vec<Rule> {
        // "Create an initially empty list of rules."
        let mut rules = Vec::new();

        loop {
            match self.peek() {
                // "Reconsume the current input token."
                // "<whitespace-token>"
                // "Do nothing."
                Some(CSSToken::Whitespace) => {
                    let _ = self.consume();
                }

                // "<EOF-token>"
                // "Return the list of rules."
                None | Some(CSSToken::EOF) => {
                    return rules;
                }

                // "<CDO-token>" or "<CDC-token>"
                Some(CSSToken::CDO) | Some(CSSToken::CDC) => {
                    if top_level {
                        // "Do nothing."
                        let _ = self.consume();
                    } else {
                        // "Reconsume the current input token. Consume a qualified rule.
                        // If anything is returned, append it to the list of rules."
                        if let Some(rule) = self.consume_qualified_rule() {
                            rules.push(Rule::Style(rule));
                        }
                    }
                }

                // "<at-keyword-token>"
                // "Reconsume the current input token. Consume an at-rule, and append
                // the returned value to the list of rules."
                Some(CSSToken::AtKeyword(_)) => {
                    if let Some(at_rule) = self.consume_at_rule() {
                        rules.push(Rule::At(at_rule));
                    }
                }

                // "anything else"
                // "Reconsume the current input token. Consume a qualified rule. If
                // anything is returned, append it to the list of rules."
                Some(_) => {
                    if let Some(rule) = self.consume_qualified_rule() {
                        rules.push(Rule::Style(rule));
                    }
                }
            }
        }
    }

    /// [§ 5.4.2 Consume an at-rule](https://www.w3.org/TR/css-syntax-3/#consume-at-rule)
    fn consume_at_rule(&mut self) -> Option<AtRule> {
        // "Consume the next input token."
        let name = match self.consume() {
            Some(CSSToken::AtKeyword(name)) => name.clone(),
            _ => return None,
        };

        // "Create a new at-rule with its name set to the value of the current input
        // token, its prelude initially set to an empty list, and its value initially
        // set to nothing."
        let mut prelude = Vec::new();

        loop {
            match self.peek() {
                // "<semicolon-token>"
                // "Return the at-rule."
                Some(CSSToken::Semicolon) => {
                    let _ = self.consume();
                    return Some(AtRule {
                        name,
                        prelude,
                        block: None,
                    });
                }

                // "<EOF-token>"
                // "This is a parse error. Return the at-rule."
                None | Some(CSSToken::EOF) => {
                    return Some(AtRule {
                        name,
                        prelude,
                        block: None,
                    });
                }

                // "<{-token>"
                // "Consume a simple block and assign it to the at-rule's block.
                // Return the at-rule."
                Some(CSSToken::LeftBrace) => {
                    let block = self.consume_simple_block();
                    return Some(AtRule {
                        name,
                        prelude,
                        block: Some(block),
                    });
                }

                // "anything else"
                // "Reconsume the current input token. Consume a component value.
                // Append the returned value to the at-rule's prelude."
                Some(_) => {
                    if let Some(value) = self.consume_component_value() {
                        prelude.push(value);
                    }
                }
            }
        }
    }

    /// [§ 5.4.3 Consume a qualified rule](https://www.w3.org/TR/css-syntax-3/#consume-qualified-rule)
    fn consume_qualified_rule(&mut self) -> Option<StyleRule> {
        // "Create a new qualified rule with its prelude initially set to an empty list,
        // and its value initially set to nothing."
        let mut prelude_tokens = Vec::new();

        loop {
            match self.peek() {
                // "<EOF-token>"
                // "This is a parse error. Return nothing."
                None | Some(CSSToken::EOF) => {
                    return None;
                }

                // "<{-token>"
                // "Consume a simple block and assign it to the qualified rule's block.
                // Return the qualified rule."
                Some(CSSToken::LeftBrace) => {
                    let _ = self.consume(); // {

                    // Parse selectors from prelude tokens, splitting on commas
                    // [§ 5.1 Selector Lists](https://www.w3.org/TR/selectors-4/#selector-list)
                    // "A selector list is a comma-separated list of selectors"
                    let selectors = split_selector_list(&prelude_tokens);

                    // Parse declarations from block contents
                    let declarations = self.consume_style_block_contents();

                    // Consume closing brace
                    if self.peek() == Some(&CSSToken::RightBrace) {
                        let _ = self.consume();
                    }

                    return Some(StyleRule {
                        selectors,
                        declarations,
                    });
                }

                // "anything else"
                // "Reconsume the current input token. Consume a component value.
                // Append the returned value to the qualified rule's prelude."
                Some(_) => {
                    if let Some(token) = self.consume().cloned() {
                        prelude_tokens.push(token);
                    }
                }
            }
        }
    }

    /// [§ 5.4.7 Consume a simple block](https://www.w3.org/TR/css-syntax-3/#consume-simple-block)
    fn consume_simple_block(&mut self) -> Vec<ComponentValue> {
        let ending_token = match self.consume() {
            Some(CSSToken::LeftBrace) => CSSToken::RightBrace,
            Some(CSSToken::LeftBracket) => CSSToken::RightBracket,
            Some(CSSToken::LeftParen) => CSSToken::RightParen,
            _ => return Vec::new(),
        };

        let mut value = Vec::new();

        loop {
            match self.peek() {
                Some(token) if *token == ending_token => {
                    let _ = self.consume();
                    return value;
                }
                None | Some(CSSToken::EOF) => {
                    return value;
                }
                Some(_) => {
                    if let Some(v) = self.consume_component_value() {
                        value.push(v);
                    }
                }
            }
        }
    }

    /// [§ 5.4.4 Consume a style block's contents](https://www.w3.org/TR/css-syntax-3/#consume-style-block)
    ///
    /// "This algorithm consumes the content of a style block..."
    fn consume_style_block_contents(&mut self) -> Vec<Declaration> {
        self.consume_list_of_declarations()
    }

    /// [§ 5.4.5 Consume a list of declarations](https://www.w3.org/TR/css-syntax-3/#consume-list-of-declarations)
    fn consume_list_of_declarations(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();

        loop {
            match self.peek() {
                // "<whitespace-token>" or "<semicolon-token>"
                // "Do nothing."
                Some(CSSToken::Whitespace) | Some(CSSToken::Semicolon) => {
                    let _ = self.consume();
                }

                // "<EOF-token>" or "<}-token>"
                // "Return the list of declarations."
                None | Some(CSSToken::EOF) | Some(CSSToken::RightBrace) => {
                    return declarations;
                }

                // "<at-keyword-token>"
                // "Reconsume the current input token. Consume an at-rule. Append the
                // returned rule to the list of declarations."
                Some(CSSToken::AtKeyword(_)) => {
                    // For simplicity, skip at-rules in declaration lists
                    let _ = self.consume_at_rule();
                }

                // "<ident-token>"
                // "Consume a declaration. If anything was returned, append it to
                // the list of declarations."
                Some(CSSToken::Ident(_)) => {
                    if let Some(decl) = self.consume_declaration() {
                        declarations.push(decl);
                    }
                }

                // "anything else"
                // "This is a parse error. Reconsume the current input token. As long as
                // the next input token is anything other than a <semicolon-token> or
                // <EOF-token>, consume a component value and throw away the returned value."
                Some(_) => {
                    let _ = self.consume();
                    while !matches!(
                        self.peek(),
                        None | Some(CSSToken::Semicolon)
                            | Some(CSSToken::RightBrace)
                            | Some(CSSToken::EOF)
                    ) {
                        let _ = self.consume_component_value();
                    }
                }
            }
        }
    }

    /// [§ 5.4.6 Consume a declaration](https://www.w3.org/TR/css-syntax-3/#consume-declaration)
    fn consume_declaration(&mut self) -> Option<Declaration> {
        // "Consume the next input token."
        let name = match self.consume() {
            Some(CSSToken::Ident(name)) => name.clone(),
            _ => return None,
        };

        // "While the next input token is a <whitespace-token>, consume the next input token."
        while self.peek() == Some(&CSSToken::Whitespace) {
            let _ = self.consume();
        }

        // "If the next input token is anything other than a <colon-token>, this is a parse error.
        // Return nothing."
        if self.peek() != Some(&CSSToken::Colon) {
            return None;
        }
        let _ = self.consume(); // :

        // "While the next input token is a <whitespace-token>, consume the next input token."
        while self.peek() == Some(&CSSToken::Whitespace) {
            let _ = self.consume();
        }

        // "As long as the next input token is anything other than an <EOF-token>, consume a
        // component value and append it to the declaration's value."
        let mut value = Vec::new();
        while !matches!(
            self.peek(),
            None | Some(CSSToken::EOF) | Some(CSSToken::Semicolon) | Some(CSSToken::RightBrace)
        ) {
            if let Some(v) = self.consume_component_value() {
                value.push(v);
            }
        }

        // Check for !important
        let important = check_important(&value);

        // Remove trailing whitespace and !important from value
        let value = trim_important(value);

        Some(Declaration {
            name,
            value,
            important,
        })
    }

    /// [§ 5.4.8 Consume a component value](https://www.w3.org/TR/css-syntax-3/#consume-component-value)
    fn consume_component_value(&mut self) -> Option<ComponentValue> {
        match self.peek() {
            // "<{-token>", "<[-token>", "<(-token>"
            Some(CSSToken::LeftBrace) | Some(CSSToken::LeftBracket) | Some(CSSToken::LeftParen) => {
                let token = match self.peek() {
                    Some(CSSToken::LeftBrace) => '{',
                    Some(CSSToken::LeftBracket) => '[',
                    Some(CSSToken::LeftParen) => '(',
                    _ => return None,
                };
                let value = self.consume_simple_block();
                Some(ComponentValue::Block { token, value })
            }

            // "<function-token>"
            Some(CSSToken::Function(_)) => {
                let name = match self.consume() {
                    Some(CSSToken::Function(name)) => name.clone(),
                    _ => return None,
                };
                let mut value = Vec::new();
                loop {
                    match self.peek() {
                        Some(CSSToken::RightParen) => {
                            let _ = self.consume();
                            break;
                        }
                        None | Some(CSSToken::EOF) => break,
                        Some(_) => {
                            if let Some(v) = self.consume_component_value() {
                                value.push(v);
                            }
                        }
                    }
                }
                Some(ComponentValue::Function { name, value })
            }

            // "anything else"
            Some(_) => {
                let token = self.consume()?.clone();
                Some(ComponentValue::Token(token))
            }

            None => None,
        }
    }

    fn consume(&mut self) -> Option<&CSSToken> {
        if self.position < self.tokens.len() {
            let token = &self.tokens[self.position];
            self.position += 1;
            Some(token)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<&CSSToken> {
        self.tokens.get(self.position)
    }
}

/// [§ 5.1 Selector Lists](https://www.w3.org/TR/selectors-4/#selector-list)
///
/// Split prelude tokens into a list of selectors, separated by commas.
/// "A selector list is a comma-separated list of selectors."
fn split_selector_list(tokens: &[CSSToken]) -> Vec<Selector> {
    let mut selectors = Vec::new();
    let mut current = Vec::new();

    for token in tokens {
        if matches!(token, CSSToken::Comma) {
            // End of current selector, start a new one
            let text = tokens_to_selector_string(&current);
            if !text.is_empty() {
                selectors.push(Selector { text });
            }
            current.clear();
        } else {
            current.push(token.clone());
        }
    }

    // Don't forget the last selector (after the last comma, or the only one)
    let text = tokens_to_selector_string(&current);
    if !text.is_empty() {
        selectors.push(Selector { text });
    }

    selectors
}

/// Convert prelude tokens to a selector string representation.
///
/// [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing) describes how CSS
/// is parsed into component values and tokens. Selectors appear as the prelude
/// of qualified rules.
///
/// [§ 9 Serialization](https://www.w3.org/TR/css-syntax-3/#serialization) defines
/// how tokens are serialized back to strings. This function performs a simplified
/// serialization for our selector matching purposes.
///
/// NOTE: This is an implementation helper, not a full spec-compliant serializer.
/// It handles the common token types found in selectors.
fn tokens_to_selector_string(tokens: &[CSSToken]) -> String {
    let mut s = String::new();
    for token in tokens {
        match token {
            CSSToken::Ident(v) => s.push_str(v),
            CSSToken::Hash { value, .. } => {
                s.push('#');
                s.push_str(value);
            }
            CSSToken::Delim(c) => s.push(*c),
            CSSToken::Colon => s.push(':'),
            CSSToken::Whitespace => s.push(' '),
            CSSToken::LeftBracket => s.push('['),
            CSSToken::RightBracket => s.push(']'),
            CSSToken::String(v) => {
                s.push('"');
                s.push_str(v);
                s.push('"');
            }
            _ => {}
        }
    }
    s.trim().to_string()
}

/// Check if the value ends with !important.
///
/// [§ 6.4.2 Important declarations](https://www.w3.org/TR/css-cascade-4/#importance)
///
/// "A declaration is important if it has a !important annotation, i.e.
/// if the last two (non-whitespace, non-comment) tokens in its value are
/// a <delim-token> with the value "!" followed by an <ident-token> with
/// a value that is an ASCII case-insensitive match for "important"."
///
/// STEP 1: Skip trailing whitespace in the value.
/// STEP 2: Check for <ident-token> "important".
/// STEP 3: Skip any whitespace between "!" and "important".
/// STEP 4: Check for <delim-token> "!".
fn check_important(value: &[ComponentValue]) -> bool {
    let mut iter = value.iter().rev().peekable();

    // STEP 1: Skip trailing whitespace
    while let Some(ComponentValue::Token(CSSToken::Whitespace)) = iter.peek() {
        let _ = iter.next();
    }

    // STEP 2: Check for ident "important"
    match iter.next() {
        Some(ComponentValue::Token(CSSToken::Ident(s))) if s.eq_ignore_ascii_case("important") => {}
        _ => return false,
    }

    // STEP 3: Skip whitespace between ! and important
    while let Some(ComponentValue::Token(CSSToken::Whitespace)) = iter.peek() {
        let _ = iter.next();
    }

    // STEP 4: Check for !
    matches!(
        iter.next(),
        Some(ComponentValue::Token(CSSToken::Delim('!')))
    )
}

/// Remove trailing whitespace and !important from a declaration value.
///
/// [§ 6.4.2 Important declarations](https://www.w3.org/TR/css-cascade-4/#importance)
///
/// After detecting !important annotation, this function removes it from the
/// value so the remaining tokens represent the actual property value.
///
/// STEP 1: Remove trailing whitespace.
/// STEP 2: Remove "important" ident token.
/// STEP 3: Remove whitespace between ! and important.
/// STEP 4: Remove "!" delim token.
/// STEP 5: Remove any remaining trailing whitespace.
fn trim_important(mut value: Vec<ComponentValue>) -> Vec<ComponentValue> {
    // STEP 1: Remove trailing whitespace
    while matches!(
        value.last(),
        Some(ComponentValue::Token(CSSToken::Whitespace))
    ) {
        let _ = value.pop();
    }

    // STEP 2: Check and remove "important"
    if matches!(
        value.last(),
        Some(ComponentValue::Token(CSSToken::Ident(s))) if s.eq_ignore_ascii_case("important")
    ) {
        let _ = value.pop();

        // STEP 3: Remove whitespace between ! and important
        while matches!(
            value.last(),
            Some(ComponentValue::Token(CSSToken::Whitespace))
        ) {
            let _ = value.pop();
        }

        // STEP 4: Remove !
        if matches!(
            value.last(),
            Some(ComponentValue::Token(CSSToken::Delim('!')))
        ) {
            let _ = value.pop();
        }
    }

    // STEP 5: Remove trailing whitespace again
    while matches!(
        value.last(),
        Some(ComponentValue::Token(CSSToken::Whitespace))
    ) {
        let _ = value.pop();
    }

    value
}
