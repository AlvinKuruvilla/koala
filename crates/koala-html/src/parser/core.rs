use strum_macros::Display;

use koala_common::diagnostics::{self, Diagnostic};
use koala_dom::{AttributesMap, DomTree, ElementData, Namespace, NodeId, NodeType};

use super::foreign_content::{
    adjust_foreign_attributes, adjust_mathml_attributes, adjust_svg_attributes, adjust_svg_tag_name,
};
use crate::tokenizer::{Attribute, Token};

/// [§ 13.2.4.1 The insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-insertion-mode)
///
/// "The insertion mode is a state variable that controls the primary operation
/// of the tree construction stage."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum InsertionMode {
    /// [§ 13.2.6.4.1 The "initial" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode)
    Initial,
    /// [§ 13.2.6.4.2 The "before html" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode)
    BeforeHtml,
    /// [§ 13.2.6.4.3 The "before head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode)
    BeforeHead,
    /// [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
    InHead,
    /// [§ 13.2.6.4.5 The "in head noscript" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inheadnoscript)
    InHeadNoscript,
    /// [§ 13.2.6.4.6 The "after head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode)
    AfterHead,
    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    InBody,
    /// [§ 13.2.6.4.8 The "text" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incdata)
    Text,
    /// [§ 13.2.6.4.9 The "in table" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
    InTable,
    /// [§ 13.2.6.4.10 The "in table text" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext)
    InTableText,
    /// [§ 13.2.6.4.11 The "in caption" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incaption)
    InCaption,
    /// [§ 13.2.6.4.12 The "in column group" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incolumngroup)
    InColumnGroup,
    /// [§ 13.2.6.4.13 The "in table body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intablebody)
    InTableBody,
    /// [§ 13.2.6.4.14 The "in row" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow)
    InRow,
    /// [§ 13.2.6.4.15 The "in cell" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incell)
    InCell,
    /// [§ 13.2.6.4.16 The "in select" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselect)
    InSelect,
    /// [§ 13.2.6.4.17 The "in select in table" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselectintable)
    InSelectInTable,
    /// [§ 13.2.6.4.18 The "in template" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intemplate)
    InTemplate,
    /// [§ 13.2.6.4.19 The "after body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterbody)
    AfterBody,
    /// [§ 13.2.6.4.20 The "in frameset" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inframeset)
    InFrameset,
    /// [§ 13.2.6.4.21 The "after frameset" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset)
    AfterFrameset,
    /// [§ 13.2.6.4.22 The "after after body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-body-insertion-mode)
    AfterAfterBody,
    /// [§ 13.2.6.4.23 The "after after frameset" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-frameset-insertion-mode)
    AfterAfterFrameset,
}

/// [§ 13.2.2 Parse errors](https://html.spec.whatwg.org/multipage/parsing.html#parse-errors)
///
/// "This specification defines the parsing rules for HTML documents...
/// The handling of parse errors is well-defined."
#[derive(Debug, Clone)]
pub struct ParseIssue {
    /// Description of the parse error per the spec's error definitions.
    pub message: String,
    /// Index into the token stream where this error was encountered.
    pub token_index: usize,
    /// "Parse errors are only errors with the content—they are not, for instance,
    /// errors in the syntax of the specification itself."
    pub is_error: bool,
}

/// [§ 13.2.4.3 The list of active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#the-list-of-active-formatting-elements)
///
/// "The list of active formatting elements... is used to handle mis-nested
/// formatting element tags."
///
/// The list contains entries that are either elements or markers.
#[derive(Debug, Clone)]
pub enum ActiveFormattingElement {
    /// A formatting element entry.
    ///
    /// "The list contains elements in the formatting category..."
    /// Formatting elements are: a, b, big, code, em, font, i, nobr, s, small,
    /// strike, strong, tt, u.
    Element {
        /// The `NodeId` of the element in the DOM tree.
        node_id: NodeId,
        /// The original token, kept to recreate the element if needed during
        /// the adoption agency algorithm or when reconstructing.
        token: Token,
    },
    /// A marker entry.
    ///
    /// "A marker is an entry in the list of active formatting elements that is
    /// distinct from any element."
    ///
    /// Markers are pushed when entering: applet, object, marquee, template,
    /// td, th, caption. They scope the list so that formatting elements from
    /// outside these elements don't affect content inside.
    Marker,
}

/// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction)
///
/// The HTML parser builds a DOM tree from a stream of tokens.
pub struct HTMLParser {
    /// [§ 13.2.4.1 The insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-insertion-mode)
    insertion_mode: InsertionMode,

    /// [§ 13.2.4.2 The original insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#original-insertion-mode)
    original_insertion_mode: Option<InsertionMode>,

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#the-stack-of-open-elements)
    ///
    /// Stores `NodeId`s into the arena.
    stack_of_open_elements: Vec<NodeId>,

    /// [§ 13.2.4.4 The element pointers](https://html.spec.whatwg.org/multipage/parsing.html#the-element-pointers)
    head_element_pointer: Option<NodeId>,

    /// [§ 13.2.4.3 The list of active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#the-list-of-active-formatting-elements)
    ///
    /// "The list of active formatting elements... is used to handle mis-nested
    /// formatting element tags."
    ///
    /// Initially, the list is empty.
    active_formatting_elements: Vec<ActiveFormattingElement>,

    /// DOM tree with parent/sibling pointers.
    /// `NodeId::ROOT` (index 0) is the Document node.
    tree: DomTree,

    /// Input tokens from the tokenizer.
    tokens: Vec<Token>,

    /// Current position in token stream.
    token_index: usize,

    /// Whether we've stopped parsing.
    stopped: bool,

    /// Parse issues (errors and warnings) encountered during parsing.
    issues: Vec<ParseIssue>,

    /// If true, panic on unhandled tokens or unexpected states.
    strict_mode: bool,

    /// [§ 13.2.6.1 Foster parenting](https://html.spec.whatwg.org/multipage/parsing.html#foster-parent)
    ///
    /// "If the foster parenting flag is set and the adjusted insertion location
    /// is inside a table, tbody, tfoot, thead, or tr element..."
    foster_parenting: bool,

    /// [§ 13.2.6.4.10 The "in table text" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext)
    ///
    /// "The pending table character tokens list"
    pending_table_character_tokens: Vec<Token>,

    /// [§ 13.2.4.4 The element pointers](https://html.spec.whatwg.org/multipage/parsing.html#form-element-pointer)
    ///
    /// "The form element pointer points to the last form element that was opened
    /// and whose end tag has not yet been seen."
    form_element_pointer: Option<NodeId>,
}

impl HTMLParser {
    /// Create a new parser from a token stream.
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        // DomTree::new() creates the Document node at NodeId::ROOT
        Self {
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: None,
            stack_of_open_elements: Vec::new(),
            head_element_pointer: None,
            active_formatting_elements: Vec::new(),
            tree: DomTree::new(),
            tokens,
            token_index: 0,
            stopped: false,
            issues: Vec::new(),
            strict_mode: false,
            foster_parenting: false,
            pending_table_character_tokens: Vec::new(),
            form_element_pointer: None,
        }
    }

    /// Enable strict mode - panics on unhandled tokens.
    #[must_use]
    pub const fn with_strict_mode(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Get all parse issues (errors and warnings) encountered during parsing.
    #[must_use]
    pub fn get_issues(&self) -> &[ParseIssue] {
        &self.issues
    }

    /// Record a parse warning (for unhandled but recoverable situations).
    ///
    /// Reports to the load's diagnostics and stores the issue for later retrieval.
    #[allow(dead_code)]
    fn parse_warning(&mut self, message: &str) {
        diagnostics::report(|| Diagnostic::HtmlParseWarning {
            message: message.to_string(),
        });
        self.issues.push(ParseIssue {
            message: message.to_string(),
            token_index: self.token_index,
            is_error: false,
        });
    }

    /// Run the parser and return the DOM tree.
    ///
    /// The returned `DomTree` preserves parent/sibling relationships
    /// for efficient traversal.
    ///
    /// # Panics
    ///
    /// Panics if the parser encounters an unimplemented insertion mode
    /// (e.g., `InTableText`, `InTableBody`, `InRow`, `InCell`,
    /// `InTemplate`, `InFrameset`).
    #[must_use]
    pub fn run(mut self) -> DomTree {
        while !self.stopped && self.token_index < self.tokens.len() {
            let token = self.tokens[self.token_index].clone();
            self.process_token(&token);
            self.token_index += 1;
        }
        self.tree
    }

    /// Run the parser and return both the `DomTree` and any parse issues.
    ///
    /// # Panics
    ///
    /// Panics if the parser encounters an unimplemented insertion mode
    /// (e.g., `InTableText`, `InTableBody`, `InRow`, `InCell`,
    /// `InTemplate`, `InFrameset`).
    #[must_use]
    pub fn run_with_issues(mut self) -> (DomTree, Vec<ParseIssue>) {
        while !self.stopped && self.token_index < self.tokens.len() {
            let token = self.tokens[self.token_index].clone();
            self.process_token(&token);
            self.token_index += 1;
        }
        let issues = std::mem::take(&mut self.issues);
        (self.tree, issues)
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction-dispatcher)
    ///
    /// The tree construction dispatcher: "As each token is emitted from the
    /// tokenizer, the user agent must follow the appropriate steps from the
    /// following list, known as the tree construction dispatcher:"
    fn process_token(&mut self, token: &Token) {
        if self.uses_html_content_rules(token) {
            // "Process the token according to the rules given in the section
            // corresponding to the current insertion mode in HTML content."
            self.process_token_in_html_content(token);
        } else {
            // "Otherwise: Process the token according to the rules given in
            // the section for parsing tokens in foreign content."
            self.process_token_in_foreign_content(token);
        }
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction-dispatcher)
    ///
    /// Whether the dispatcher sends `token` to the insertion mode rules
    /// rather than the foreign content rules.
    fn uses_html_content_rules(&self, token: &Token) -> bool {
        // "If the stack of open elements is empty"
        let Some(node) = self.adjusted_current_node() else {
            return true;
        };
        let namespace = self.namespace_of(node);
        let tag_name = self.get_tag_name(node).unwrap_or_default();
        let is_start_tag = |names: &[&str]| {
            matches!(token, Token::StartTag { name, .. } if names.contains(&name.as_str()))
        };
        let is_character = matches!(token, Token::Character { .. });

        // "If the adjusted current node is an element in the HTML namespace"
        namespace == Some(Namespace::Html)
            // "If the adjusted current node is a MathML text integration point
            // and the token is a start tag whose tag name is neither "mglyph"
            // nor "malignmark""
            || (self.is_mathml_text_integration_point(node)
                && matches!(token, Token::StartTag { .. })
                && !is_start_tag(&["mglyph", "malignmark"]))
            // "If the adjusted current node is a MathML text integration point
            // and the token is a character token"
            || (self.is_mathml_text_integration_point(node) && is_character)
            // "If the adjusted current node is a MathML annotation-xml element
            // and the token is a start tag whose tag name is "svg""
            || (namespace == Some(Namespace::MathMl)
                && tag_name == "annotation-xml"
                && is_start_tag(&["svg"]))
            // "If the adjusted current node is an HTML integration point and
            // the token is a start tag"
            || (self.is_html_integration_point(node) && matches!(token, Token::StartTag { .. }))
            // "If the adjusted current node is an HTML integration point and
            // the token is a character token"
            || (self.is_html_integration_point(node) && is_character)
            // "If the token is an end-of-file token"
            || matches!(token, Token::EndOfFile)
    }

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#adjusted-current-node)
    ///
    /// "The adjusted current node is the parser's fragment context element if
    /// that element is non-null and the stack of open elements has only one
    /// element in it (fragment case); otherwise, the adjusted current node is
    /// the current node."
    ///
    /// Implementation note: Koala does not implement the fragment parsing
    /// algorithm, so there is no context element and this is always the
    /// current node.
    fn adjusted_current_node(&self) -> Option<NodeId> {
        self.current_node()
    }

    /// The namespace of the element `id`, or `None` if it is not an element.
    fn namespace_of(&self, id: NodeId) -> Option<Namespace> {
        self.tree.as_element(id).map(|data| data.namespace)
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#mathml-text-integration-point)
    ///
    /// "A node is a MathML text integration point if it is one of the
    /// following elements: A MathML mi element, A MathML mo element, A MathML
    /// mn element, A MathML ms element, A MathML mtext element"
    fn is_mathml_text_integration_point(&self, id: NodeId) -> bool {
        self.namespace_of(id) == Some(Namespace::MathMl)
            && matches!(
                self.get_tag_name(id),
                Some("mi" | "mo" | "mn" | "ms" | "mtext")
            )
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#html-integration-point)
    ///
    /// "A node is an HTML integration point if it is one of the following
    /// elements:"
    fn is_html_integration_point(&self, id: NodeId) -> bool {
        let Some(element) = self.tree.as_element(id) else {
            return false;
        };
        match element.namespace {
            // "A MathML annotation-xml element whose start tag token had an
            // attribute with the name "encoding" whose value was an ASCII
            // case-insensitive match for "text/html""
            //
            // "A MathML annotation-xml element whose start tag token had an
            // attribute with the name "encoding" whose value was an ASCII
            // case-insensitive match for "application/xhtml+xml""
            //
            // Implementation note: the element's attributes are the start
            // tag's, since nothing can change them before this check.
            Namespace::MathMl => {
                element.tag_name.as_str() == "annotation-xml"
                    && element.attrs.get("encoding").is_some_and(|encoding| {
                        encoding.eq_ignore_ascii_case("text/html")
                            || encoding.eq_ignore_ascii_case("application/xhtml+xml")
                    })
            }
            // "An SVG foreignObject element", "An SVG desc element", "An SVG
            // title element"
            Namespace::Svg => {
                matches!(element.tag_name.as_str(), "foreignObject" | "desc" | "title")
            }
            Namespace::Html => false,
        }
    }

    /// [§ 13.2.6.5 The rules for parsing tokens in foreign content](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inforeign)
    ///
    /// "When the user agent is to apply the rules for parsing tokens in
    /// foreign content, the user agent must handle the token as follows:"
    fn process_token_in_foreign_content(&mut self, token: &Token) {
        match token {
            // "A character token that is U+0000 NULL"
            // "Parse error. Insert a U+FFFD REPLACEMENT CHARACTER character."
            Token::Character { data: '\0' } => self.insert_character('\u{FFFD}'),

            // "A character token that is one of U+0009 CHARACTER TABULATION,
            // U+000A LINE FEED (LF), U+000C FORM FEED (FF), U+000D CARRIAGE
            // RETURN (CR), or U+0020 SPACE"
            // "Insert the token's character."
            //
            // "Any other character token"
            // "Insert the token's character. Set the frameset-ok flag to "not
            // ok"."
            //
            // TODO: the parser has no frameset-ok flag yet (see the TODO in
            // "in body"), so both arms only insert the character.
            Token::Character { data } => self.insert_character(*data),

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => self.insert_comment(data),

            // "A processing instruction token" never occurs: Koala's
            // tokenizer emits processing instructions as bogus comments.

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

            // "A start tag whose tag name is one of: "b", "big", "blockquote",
            // "body", "br", "center", "code", "dd", "div", "dl", "dt", "em",
            // "embed", "h1", "h2", "h3", "h4", "h5", "h6", "head", "hr", "i",
            // "img", "li", "listing", "menu", "meta", "nobr", "ol", "p", "pre",
            // "ruby", "s", "small", "span", "strong", "strike", "sub", "sup",
            // "table", "tt", "u", "ul", "var""
            //
            // "A start tag whose tag name is "font", if the token has any
            // attributes named "color", "face", or "size""
            //
            // "An end tag whose tag name is "br", "p""
            token if Self::breaks_out_of_foreign_content(token) => {
                // "Parse error."
                //
                // "While the current node is not a MathML text integration
                // point, an HTML integration point, or an element in the HTML
                // namespace, pop elements from the stack of open elements."
                while let Some(node) = self.current_node() {
                    if self.is_mathml_text_integration_point(node)
                        || self.is_html_integration_point(node)
                        || self.namespace_of(node) == Some(Namespace::Html)
                    {
                        break;
                    }
                    let _ = self.stack_of_open_elements.pop();
                }
                // "Reprocess the token according to the rules given in the
                // section corresponding to the current insertion mode in HTML
                // content."
                self.process_token_in_html_content(token);
            }

            // "Any other start tag"
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let namespace = self
                    .adjusted_current_node()
                    .and_then(|node| self.namespace_of(node))
                    .expect("foreign content rules only run with a foreign element open");
                let mut name = name.clone();
                let mut attributes = attributes.clone();

                // "If the adjusted current node is an element in the MathML
                // namespace, adjust MathML attributes for the token."
                if namespace == Namespace::MathMl {
                    adjust_mathml_attributes(&mut attributes);
                }
                if namespace == Namespace::Svg {
                    // "If the adjusted current node is an element in the SVG
                    // namespace, and the token's tag name is one of the ones
                    // in the first column of the following table, change the
                    // tag name to the name given in the corresponding cell in
                    // the second column."
                    name = adjust_svg_tag_name(&name).to_string();
                    // "If the adjusted current node is an element in the SVG
                    // namespace, adjust SVG attributes for the token."
                    adjust_svg_attributes(&mut attributes);
                }
                // "Adjust foreign attributes for the token."
                adjust_foreign_attributes(&mut attributes);

                // "Insert a foreign element for the token, with the adjusted
                // current node's namespace and false."
                let adjusted_token = Token::StartTag {
                    name: name.clone(),
                    attributes,
                    self_closing: *self_closing,
                };
                let _ = self.insert_foreign_element(&adjusted_token, namespace);

                // "If the token has its self-closing flag set, then run the
                // appropriate steps from the following list:"
                if *self_closing {
                    // "If the token's tag name is "script", and the new
                    // current node is in the SVG namespace: Acknowledge the
                    // token's self-closing flag, and then act as described in
                    // the steps for a "script" end tag below."
                    //
                    // "Otherwise: Pop the current node off the stack of open
                    // elements and acknowledge the token's self-closing flag."
                    //
                    // Both pop the new element; the script steps would also
                    // run it, which Koala does not do (see the "script" end
                    // tag arm).
                    let _ = self.stack_of_open_elements.pop();
                }
            }

            // "An end tag whose tag name is "script", if the current node is an
            // SVG script element"
            Token::EndTag { name, .. }
                if name == "script"
                    && self.current_node().is_some_and(|node| {
                        self.namespace_of(node) == Some(Namespace::Svg)
                            && self.get_tag_name(node) == Some("script")
                    }) =>
            {
                // "Pop the current node off the stack of open elements."
                let _ = self.stack_of_open_elements.pop();
                // TODO: the remaining steps pause the parser and "Process the
                // SVG script element according to the SVG rules". Koala does
                // not run SVG scripts, so the element is left inert.
            }

            // "Any other end tag"
            Token::EndTag { name, .. } => {
                // "Initialize node to be the current node (the bottommost node
                // of the stack)."
                let mut index = self.stack_of_open_elements.len() - 1;
                // "If node's tag name, converted to ASCII lowercase, is not the
                // same as the tag name of the token, then this is a parse
                // error."
                loop {
                    let node = self.stack_of_open_elements[index];
                    // "Loop: If node is the topmost element in the stack of
                    // open elements, then return. (fragment case)"
                    if index == 0 {
                        return;
                    }
                    // "If node's tag name, converted to ASCII lowercase, is the
                    // same as the tag name of the token, pop elements from the
                    // stack of open elements until node has been popped from
                    // the stack, and then return."
                    if self
                        .get_tag_name(node)
                        .is_some_and(|tag| tag.to_ascii_lowercase() == *name)
                    {
                        self.stack_of_open_elements.truncate(index);
                        return;
                    }
                    // "Set node to the previous entry in the stack of open
                    // elements."
                    index -= 1;
                    // "If node is not an element in the HTML namespace, return
                    // to the step labeled loop."
                    if self.namespace_of(self.stack_of_open_elements[index])
                        != Some(Namespace::Html)
                    {
                        continue;
                    }
                    // "Otherwise, process the token according to the rules
                    // given in the section corresponding to the current
                    // insertion mode in HTML content."
                    self.process_token_in_html_content(token);
                    return;
                }
            }

            // The dispatcher sends end-of-file to the insertion mode rules.
            Token::EndOfFile => unreachable!("the dispatcher handles end-of-file in HTML content"),
        }
    }

    /// Whether `token` is one of the tokens that leave foreign content:
    /// the start tags and end tags listed in the arm that calls this.
    fn breaks_out_of_foreign_content(token: &Token) -> bool {
        const BREAKOUT_START_TAGS: &[&str] = &[
            "b", "big", "blockquote", "body", "br", "center", "code", "dd", "div", "dl", "dt",
            "em", "embed", "h1", "h2", "h3", "h4", "h5", "h6", "head", "hr", "i", "img", "li",
            "listing", "menu", "meta", "nobr", "ol", "p", "pre", "ruby", "s", "small", "span",
            "strong", "strike", "sub", "sup", "table", "tt", "u", "ul", "var",
        ];
        match token {
            Token::StartTag {
                name, attributes, ..
            } => {
                BREAKOUT_START_TAGS.contains(&name.as_str())
                    || (name == "font"
                        && attributes
                            .iter()
                            .any(|attr| matches!(attr.name.as_str(), "color" | "face" | "size")))
            }
            Token::EndTag { name, .. } => name == "br" || name == "p",
            _ => false,
        }
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction-dispatcher)
    ///
    /// Process `token` according to the rules for the current insertion mode
    /// in HTML content.
    ///
    /// # Panics
    ///
    /// Panics if the parser encounters an unimplemented insertion mode.
    fn process_token_in_html_content(&mut self, token: &Token) {
        match self.insertion_mode {
            InsertionMode::Initial => self.handle_initial_mode(token),
            InsertionMode::BeforeHtml => self.handle_before_html_mode(token),
            InsertionMode::BeforeHead => self.handle_before_head_mode(token),
            InsertionMode::InHead => self.handle_in_head_mode(token),
            InsertionMode::InHeadNoscript => {
                self.handle_in_head_noscript_mode(token);
            }
            InsertionMode::AfterHead => self.handle_after_head_mode(token),
            InsertionMode::InBody => self.handle_in_body_mode(token),
            InsertionMode::Text => self.handle_text_mode(token),

            // ===== TABLE PARSING MODES =====
            // [§ 13.2.6.4.9-15](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
            //
            // Table parsing is complex due to:
            // - Foster parenting (misplaced content moves outside table)
            // - Pending table character tokens
            // - Multiple nested table elements (table, tbody, tr, td, th, caption, colgroup)
            //
            // STEP 1: InTable mode (IMPLEMENTED)
            //   [§ 13.2.6.4.9](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
            //   - Handles <caption>, <colgroup>, <col>, <tbody>/<tfoot>/<thead>,
            //     <td>/<th>/<tr>, <table>, </table>, <style>/<script>/<template>,
            //     </template>, <input type=hidden>, <form>, EOF
            //   - Foster parenting for misplaced content ("anything else")
            InsertionMode::InTable => self.handle_in_table_mode(token),

            // STEP 2: InTableText mode - accumulates character tokens in table context
            //   [§ 13.2.6.4.10](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext)
            //   - Accumulate character tokens
            //   - On other token: flush characters (foster parent if non-whitespace)
            InsertionMode::InTableText => self.handle_in_table_text_mode(token),

            // STEP 3: InCaption mode - handles content inside <caption>
            //   [§ 13.2.6.4.11](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incaption)
            InsertionMode::InCaption => todo!("InCaption mode - see STEP 3 above"),

            // STEP 4: InColumnGroup mode - handles <col> elements
            //   [§ 13.2.6.4.12](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incolumngroup)
            InsertionMode::InColumnGroup => todo!("InColumnGroup mode - see STEP 4 above"),

            // STEP 5: InTableBody mode - handles <tbody>, <thead>, <tfoot>
            //   [§ 13.2.6.4.13](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intablebody)
            //   - "A start tag whose tag name is 'tr'" -> insert element, switch to InRow
            //   - "A start tag whose tag name is one of: 'th', 'td'" -> act as if 'tr', reprocess
            InsertionMode::InTableBody => self.handle_in_table_body_mode(token),

            // STEP 6: InRow mode - handles <tr> and its children
            //   [§ 13.2.6.4.14](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow)
            //   - "A start tag whose tag name is one of: 'th', 'td'" -> switch to InCell
            InsertionMode::InRow => self.handle_in_row_mode(token),

            // STEP 7: InCell mode - handles <td> and <th> content
            //   [§ 13.2.6.4.15](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incell)
            //   - Process most tokens using InBody rules
            //   - Special handling for table-related end tags
            InsertionMode::InCell => self.handle_in_cell_mode(token),

            // ===== FORM ELEMENT MODES =====
            //
            // STEP 8: InSelect mode - handles <select> and <option>/<optgroup>
            //   [§ 13.2.6.4.16](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselect)
            InsertionMode::InSelect => todo!("InSelect mode - see STEP 8 above"),

            // STEP 9: InSelectInTable mode - select inside table context
            //   [§ 13.2.6.4.17](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselectintable)
            InsertionMode::InSelectInTable => todo!("InSelectInTable mode - see STEP 9 above"),

            // STEP 10: InTemplate mode - handles <template> content
            //   [§ 13.2.6.4.18](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intemplate)
            //   - Uses a stack of template insertion modes
            InsertionMode::InTemplate => todo!("InTemplate mode - see STEP 10 above"),
            InsertionMode::AfterBody => self.handle_after_body_mode(token),

            // ===== FRAMESET MODES (Low Priority - rarely used) =====
            //
            // STEP 11: InFrameset mode - handles <frameset> and <frame>
            //   [§ 13.2.6.4.20](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inframeset)
            InsertionMode::InFrameset => todo!("InFrameset mode - see STEP 11 above"),

            // STEP 12: AfterFrameset mode - after </frameset>
            //   [§ 13.2.6.4.21](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset)
            InsertionMode::AfterFrameset => todo!("AfterFrameset mode - see STEP 12 above"),

            InsertionMode::AfterAfterBody => self.handle_after_after_body_mode(token),

            // STEP 13: AfterAfterFrameset mode - final frameset state
            //   [§ 13.2.6.4.23](https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-frameset-insertion-mode)
            InsertionMode::AfterAfterFrameset => {
                todo!("AfterAfterFrameset mode - see STEP 13 above")
            }
        }
    }

    /// [§ 13.2.6 Tree construction](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction)
    ///
    /// "Reprocess the token" - process the same token again in a new insertion mode.
    /// Used when switching modes requires the current token to be handled differently.
    fn reprocess_token(&mut self, token: &Token) {
        self.process_token(token);
    }

    /// [§ 12.1.4 ASCII whitespace](https://infra.spec.whatwg.org/#ascii-whitespace)
    ///
    /// "ASCII whitespace is U+0009 TAB, U+000A LF, U+000C FF, U+000D CR,
    /// or U+0020 SPACE."
    const fn is_whitespace(c: char) -> bool {
        matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ')
    }

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#current-node)
    ///
    /// "The current node is the bottommost node in this stack of open elements."
    fn current_node(&self) -> Option<NodeId> {
        self.stack_of_open_elements.last().copied()
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#foster-parent)
    ///
    /// "If the foster parenting flag is set and the adjusted insertion location
    /// is inside a table, tbody, tfoot, thead, or tr element..."
    ///
    /// Returns `(parent_id, Option<before_id>)`. When `before_id` is `Some`,
    /// the caller must use `insert_before` instead of `append_child`.
    fn foster_parent_location(&self) -> (NodeId, Option<NodeId>) {
        // STEP 1: "Let last table be the last table element in the stack of
        //          open elements, if any."
        let last_table_pos = self
            .stack_of_open_elements
            .iter()
            .rposition(|&id| self.get_tag_name(id) == Some("table"));

        let Some(table_pos) = last_table_pos else {
            // STEP 3: "If there is no last table element in the stack of open
            //          elements, then the adjusted insertion location is inside
            //          the first element in the stack of open elements (the html
            //          element)."
            let first = self
                .stack_of_open_elements
                .first()
                .copied()
                .unwrap_or(NodeId::ROOT);
            return (first, None);
        };

        let table_id = self.stack_of_open_elements[table_pos];

        // STEP 2: "If last table has a parent node, then let adjusted
        //          insertion location be before last table in its parent
        //          node."
        self.tree.parent(table_id).map_or_else(
            || {
                // "Otherwise, let adjusted insertion location be inside the
                //  element immediately above last table in the stack of open
                //  elements."
                let above_table = self.stack_of_open_elements[table_pos - 1];
                (above_table, None)
            },
            |parent_id| (parent_id, Some(table_id)),
        )
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#appropriate-place-for-inserting-a-node)
    ///
    /// "The appropriate place for inserting a node."
    ///
    /// When foster parenting is active and the current node is a table-related
    /// element, delegates to `foster_parent_location()`. Otherwise returns the
    /// current node with no insert-before target.
    fn adjusted_insertion_location(&self) -> (NodeId, Option<NodeId>) {
        let target = self.current_node().unwrap_or(NodeId::ROOT);

        // "If foster parenting is enabled and the target is a table, tbody,
        //  tfoot, thead, or tr element..."
        if self.foster_parenting
            && let Some(tag) = self.get_tag_name(target)
            && matches!(tag, "table" | "tbody" | "tfoot" | "thead" | "tr")
        {
            return self.foster_parent_location();
        }

        (target, None)
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)
    ///
    /// Convert token attributes to the `AttributesMap` used by `ElementData`.
    fn attributes_to_map(attributes: &[Attribute]) -> AttributesMap {
        attributes
            .iter()
            .map(|attr| (attr.name.clone(), attr.value.clone()))
            .collect()
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#create-an-element-for-the-token)
    ///
    /// "To create an element for a token, given a token token, a string
    /// namespace, and a Node object intendedParent"
    ///
    /// Creates a new element node in the DOM arena.
    /// NOTE: This is a simplified version; the full algorithm also handles
    /// custom elements and the "will execute script" flag.
    fn create_element(
        &mut self,
        namespace: Namespace,
        tag_name: &str,
        attributes: &[Attribute],
    ) -> NodeId {
        self.tree.alloc(NodeType::Element(ElementData {
            tag_name: tag_name.into(),
            attrs: Self::attributes_to_map(attributes),
            namespace,
        }))
    }

    /// [§ 13.2.6.1 Insert a character](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-character)
    ///
    /// Create a Text node with the given data.
    fn create_text_node(&mut self, data: String) -> NodeId {
        self.tree.alloc(NodeType::Text(data))
    }

    /// [§ 13.2.6.1 Insert a comment](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-comment)
    ///
    /// Create a Comment node with the given data.
    fn create_comment_node(&mut self, data: String) -> NodeId {
        self.tree.alloc(NodeType::Comment(data))
    }

    /// [§ 4.2.2 Append](https://dom.spec.whatwg.org/#concept-node-append)
    ///
    /// "To append a node to a parent, pre-insert node into parent before null."
    fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.tree.append_child(parent_id, child_id);
    }

    /// [§ 13.2.6.1 Insert a character](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-character)
    ///
    /// "When the steps below require the user agent to insert a character
    /// while processing a token, the user agent must run the following steps..."
    fn insert_character(&mut self, c: char) {
        // STEP 1: "Let the adjusted insertion location be the appropriate place
        //         for inserting a node."
        let (parent_id, before_id) = self.adjusted_insertion_location();

        // STEP 2: "If there is a Text node immediately before the adjusted
        //         insertion location, then append data to that Text node's data."
        //
        // When foster parenting with insert-before, check the sibling before
        // the reference node. Otherwise check the last child of the parent.
        let adjacent_text_id = if let Some(ref_id) = before_id {
            // Find the node just before the reference in the parent's children.
            let children = self.tree.children(parent_id);
            let ref_pos = children.iter().position(|&id| id == ref_id);
            ref_pos.and_then(|pos| {
                if pos > 0 {
                    Some(children[pos - 1])
                } else {
                    None
                }
            })
        } else {
            self.tree.children(parent_id).last().copied()
        };

        if let Some(text_node_id) = adjacent_text_id
            && let Some(arena_node) = self.tree.get_mut(text_node_id)
            && let NodeType::Text(ref mut text_data) = arena_node.node_type
        {
            text_data.push(c);
            return;
        }

        // STEP 3: "Otherwise, create a new Text node whose data is data and
        //         whose node document is the same as that of the element in
        //         which the adjusted insertion location finds itself, and
        //         insert the newly created node at the adjusted insertion location."
        let text_id = self.create_text_node(String::from(c));
        if let Some(ref_id) = before_id {
            self.tree.insert_before(parent_id, text_id, ref_id);
        } else {
            self.append_child(parent_id, text_id);
        }
    }

    /// [§ 13.2.6.1 Insert a comment](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-comment)
    ///
    /// "When the steps below require the user agent to insert a comment
    /// while processing a comment token, optionally with an explicitly
    /// insertion position position..."
    fn insert_comment(&mut self, data: &str) {
        // STEP 1: "Let the adjusted insertion location be the appropriate place
        //         for inserting a node."
        let (parent_id, before_id) = self.adjusted_insertion_location();
        // STEP 2: "Create a Comment node..."
        let comment_id = self.create_comment_node(data.to_string());
        // STEP 3: "Insert the newly created node at the adjusted insertion location."
        if let Some(ref_id) = before_id {
            self.tree.insert_before(parent_id, comment_id, ref_id);
        } else {
            self.append_child(parent_id, comment_id);
        }
    }

    /// [§ 13.2.6.1 Insert a comment](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-comment)
    ///
    /// Insert a comment as the last child of the Document node.
    /// Used for comments that appear after </html>.
    fn insert_comment_to_document(&mut self, data: &str) {
        let comment_id = self.create_comment_node(data.to_string());
        self.append_child(NodeId::ROOT, comment_id);
    }

    /// [§ 13.2.6.1 Insert an HTML element](https://html.spec.whatwg.org/multipage/parsing.html#insert-an-html-element)
    ///
    /// "To insert an HTML element given a token token: insert a foreign
    /// element given token, the HTML namespace, and false."
    fn insert_html_element(&mut self, token: &Token) -> NodeId {
        self.insert_foreign_element(token, Namespace::Html)
    }

    /// [§ 13.2.6.1 Insert a foreign element](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-foreign-element)
    ///
    /// "To insert a foreign element, given a token token, a string
    /// namespace, and a boolean onlyAddToElementStack:"
    ///
    /// Implementation note: onlyAddToElementStack is always false here; the
    /// spec passes true only when parsing declarative shadow roots, which
    /// Koala does not support.
    ///
    /// # Panics
    ///
    /// Panics if called with a non-`StartTag` token, indicating a parser bug.
    fn insert_foreign_element(&mut self, token: &Token, namespace: Namespace) -> NodeId {
        let Token::StartTag {
            name, attributes, ..
        } = token
        else {
            panic!("insert_foreign_element called with non-StartTag token");
        };

        // STEP 1: "Let adjustedInsertionLocation be the appropriate place
        //         for inserting a node."
        let (parent_id, before_id) = self.adjusted_insertion_location();

        // STEP 2: "Let element be the result of creating an element for the
        //         token given token, namespace, and adjustedInsertionLocation's
        //         target parent."
        let element_id = self.create_element(namespace, name, attributes);

        // STEP 3: "If onlyAddToElementStack is false, then run insert an
        //         element at the adjusted insertion location with element."
        if let Some(ref_id) = before_id {
            self.tree.insert_before(parent_id, element_id, ref_id);
        } else {
            self.append_child(parent_id, element_id);
        }

        // STEP 4: "Push element onto the stack of open elements so that it
        //         is the new current node."
        self.stack_of_open_elements.push(element_id);

        // STEP 5: "Return element."
        element_id
    }

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#the-stack-of-open-elements)
    ///
    /// Get the tag name of a node (local name of the element).
    fn get_tag_name(&self, id: NodeId) -> Option<&str> {
        self.tree.as_element(id).map(|data| data.tag_name.as_str())
    }

    /// [§ 13.2.4.2 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#the-stack-of-open-elements)
    ///
    /// Pop elements from the stack of open elements until we find one
    /// with the given tag name (inclusive). This is a common operation
    /// referenced throughout § 13.2.6 tree construction.
    ///
    /// STEP 1: Pop the current node from the stack.
    /// STEP 2: If popped node matches target tag name, stop.
    /// STEP 3: Otherwise, repeat from STEP 1.
    fn pop_until_tag(&mut self, tag_name: &str) {
        while let Some(id) = self.stack_of_open_elements.pop() {
            // STEP 2: Check if we've reached the target element
            if self.get_tag_name(id) == Some(tag_name) {
                break;
            }
            // STEP 3: Continue popping
        }
    }

    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// Pop elements until one of the given tag names is found (inclusive).
    ///
    /// Used for heading elements per spec: "If the stack of open elements has
    /// an h1, h2, h3, h4, h5, or h6 element in scope, then...pop elements from
    /// the stack of open elements until an h1, h2, h3, h4, h5, or h6 element
    /// has been popped from the stack."
    ///
    /// STEP 1: Pop the current node from the stack.
    /// STEP 2: If popped node matches any target tag name, stop.
    /// STEP 3: Otherwise, repeat from STEP 1.
    fn pop_until_one_of(&mut self, tag_names: &[&str]) {
        while let Some(idx) = self.stack_of_open_elements.pop() {
            if let Some(name) = self.get_tag_name(idx) {
                // STEP 2: Check if we've reached any of the target elements
                if tag_names.contains(&name) {
                    break;
                }
            }
            // STEP 3: Continue popping
        }
    }

    /// [§ 13.2.4.2 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-scope)
    ///
    /// "The stack of open elements is said to have an element target node in a
    /// specific scope consisting of a list of element types list when the
    /// following algorithm terminates in a match state:"
    ///
    /// STEP 1: "Initialize node to be the current node (the bottommost node
    ///          of the stack)."
    ///
    /// STEP 2: "If node is the target node, terminate in a match state."
    ///
    /// STEP 3: "Otherwise, if node is one of the element types in list,
    ///          terminate in a failure state."
    ///
    /// STEP 4: "Otherwise, set node to the previous entry in the stack of
    ///          open elements and return to step 2."
    fn has_element_in_specific_scope(&self, tag_name: &str, scope_markers: &[&str]) -> bool {
        // Walk the stack from top (current node) downward.
        for &node_id in self.stack_of_open_elements.iter().rev() {
            if let Some(node_tag) = self.get_tag_name(node_id) {
                // STEP 2: If node is the target, match.
                if node_tag == tag_name {
                    return true;
                }
                // STEP 3: If node is a scope marker, failure.
                if scope_markers.contains(&node_tag) {
                    return false;
                }
            }
            // STEP 4: Continue to previous entry.
        }
        false
    }

    /// [§ 13.2.4.2](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-scope)
    ///
    /// "has an element in scope" (default scope).
    ///
    /// Scope markers: applet, caption, html, table, td, th, marquee, object, template
    /// (plus MathML/SVG markers omitted for now)
    fn has_element_in_scope(&self, tag_name: &str) -> bool {
        const DEFAULT_SCOPE: &[&str] = &[
            "applet", "caption", "html", "table", "td", "th", "marquee", "object",
            "template",
            // MathML: mi, mo, mn, ms, mtext, annotation-xml
            // SVG: foreignObject, desc, title
        ];
        self.has_element_in_specific_scope(tag_name, DEFAULT_SCOPE)
    }

    /// [§ 13.2.4.2](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-button-scope)
    ///
    /// "has an element in button scope" — default scope markers plus button.
    fn has_element_in_button_scope(&self, tag_name: &str) -> bool {
        const BUTTON_SCOPE: &[&str] = &[
            "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
            "button",
        ];
        self.has_element_in_specific_scope(tag_name, BUTTON_SCOPE)
    }

    /// [§ 13.2.4.2](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-list-item-scope)
    ///
    /// "has an element in list item scope" — default scope markers plus ol, ul.
    fn has_element_in_list_item_scope(&self, tag_name: &str) -> bool {
        const LIST_ITEM_SCOPE: &[&str] = &[
            "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
            "ol", "ul",
        ];
        self.has_element_in_specific_scope(tag_name, LIST_ITEM_SCOPE)
    }

    /// [§ 13.2.4.2](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-table-scope)
    ///
    /// "has an element in table scope" — scope markers: html, table, template.
    fn has_element_in_table_scope(&self, tag_name: &str) -> bool {
        const TABLE_SCOPE: &[&str] = &["html", "table", "template"];
        self.has_element_in_specific_scope(tag_name, TABLE_SCOPE)
    }

    /// [§ 13.2.6.4.9 Clear the stack back to a table context](https://html.spec.whatwg.org/multipage/parsing.html#clear-the-stack-back-to-a-table-context)
    ///
    /// "When the steps above require the UA to clear the stack back to a table
    /// context, it means that the UA must, while the current node is not a
    /// table, template, or html element, pop elements from the stack of open
    /// elements."
    fn clear_stack_back_to_table_context(&mut self) {
        while let Some(&current) = self.stack_of_open_elements.last() {
            if let Some(tag) = self.get_tag_name(current)
                && matches!(tag, "table" | "template" | "html")
            {
                break;
            }
            let _ = self.stack_of_open_elements.pop();
        }
    }

    /// [§ 13.2.6.4.13 Clear the stack back to a table body context](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intablebody)
    ///
    /// "When the steps above require the UA to clear the stack back to a table
    /// body context, it means that the UA must, while the current node is not a
    /// tbody, tfoot, thead, template, or html element, pop elements from the
    /// stack of open elements."
    fn clear_stack_back_to_table_body_context(&mut self) {
        while let Some(&current) = self.stack_of_open_elements.last() {
            if let Some(tag) = self.get_tag_name(current)
                && matches!(tag, "tbody" | "tfoot" | "thead" | "template" | "html")
            {
                break;
            }
            let _ = self.stack_of_open_elements.pop();
        }
    }

    /// [§ 13.2.6.4.14 Clear the stack back to a table row context](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow)
    ///
    /// "When the steps above require the UA to clear the stack back to a table
    /// row context, it means that the UA must, while the current node is not a
    /// tr, template, or html element, pop elements from the stack of open
    /// elements."
    fn clear_stack_back_to_table_row_context(&mut self) {
        while let Some(&current) = self.stack_of_open_elements.last() {
            if let Some(tag) = self.get_tag_name(current)
                && matches!(tag, "tr" | "template" | "html")
            {
                break;
            }
            let _ = self.stack_of_open_elements.pop();
        }
    }

    /// [§ 13.2.6.4.15 Close the cell](https://html.spec.whatwg.org/multipage/parsing.html#close-the-cell)
    ///
    /// "Where the steps above say to close the cell, they mean to run the
    /// following algorithm:"
    fn close_the_cell(&mut self) {
        // "Generate implied end tags."
        self.generate_implied_end_tags();
        // "If the current node is not now a td element or a th element,
        //  then this is a parse error."
        // "Pop elements from the stack of open elements stack until a td
        //  element or a th element has been popped from the stack."
        self.pop_until_one_of(&["td", "th"]);
        // "Clear the list of active formatting elements up to the last marker."
        self.clear_active_formatting_elements_to_last_marker();
        // "Switch the insertion mode to "in row"."
        self.insertion_mode = InsertionMode::InRow;
    }

    /// [§ 13.2.4.1 Reset the insertion mode appropriately](https://html.spec.whatwg.org/multipage/parsing.html#reset-the-insertion-mode-appropriately)
    ///
    /// "When the steps below require the UA to reset the insertion mode
    /// appropriately, the UA must follow these steps:"
    fn reset_insertion_mode_appropriately(&mut self) {
        // STEP 1: "Let last be false."
        let mut last = false;

        // STEP 2: "Let node be the last node in the stack of open elements."
        let mut node_index = self.stack_of_open_elements.len();

        loop {
            if node_index == 0 {
                break;
            }
            node_index -= 1;

            let node_id = self.stack_of_open_elements[node_index];

            // STEP 3: "If node is the first node in the stack of open elements,
            //          then set last to true..."
            if node_index == 0 {
                last = true;
                // NOTE: Fragment case would set node to context element here.
            }

            let Some(tag) = self.get_tag_name(node_id) else {
                continue;
            };

            match tag {
                // "If node is a td or th element and last is false, then switch
                //  the insertion mode to "in cell" and return."
                "td" | "th" if !last => {
                    self.insertion_mode = InsertionMode::InCell;
                    return;
                }
                // "If node is a tr element, then switch the insertion mode to
                //  "in row" and return."
                "tr" => {
                    self.insertion_mode = InsertionMode::InRow;
                    return;
                }
                // "If node is a tbody, thead, or tfoot element, then switch the
                //  insertion mode to "in table body" and return."
                "tbody" | "thead" | "tfoot" => {
                    self.insertion_mode = InsertionMode::InTableBody;
                    return;
                }
                // "If node is a caption element, then switch the insertion mode
                //  to "in caption" and return."
                "caption" => {
                    self.insertion_mode = InsertionMode::InCaption;
                    return;
                }
                // "If node is a colgroup element, then switch the insertion mode
                //  to "in column group" and return."
                "colgroup" => {
                    self.insertion_mode = InsertionMode::InColumnGroup;
                    return;
                }
                // "If node is a table element, then switch the insertion mode to
                //  "in table" and return."
                "table" => {
                    self.insertion_mode = InsertionMode::InTable;
                    return;
                }
                // "If node is a template element, then switch the insertion mode
                //  to "in template" and return."
                // NOTE: InTemplate is not yet implemented.
                "template" => {
                    self.insertion_mode = InsertionMode::InTemplate;
                    return;
                }
                // "If node is a head element and last is false, then switch the
                //  insertion mode to "in head" and return."
                "head" if !last => {
                    self.insertion_mode = InsertionMode::InHead;
                    return;
                }
                // "If node is a body element, then switch the insertion mode to
                //  "in body" and return."
                "body" => {
                    self.insertion_mode = InsertionMode::InBody;
                    return;
                }
                // "If node is an html element, then:
                //  If the head element pointer is null, switch to "before head".
                //  Otherwise, switch to "after head". Return."
                "html" => {
                    if self.head_element_pointer.is_none() {
                        self.insertion_mode = InsertionMode::BeforeHead;
                    } else {
                        self.insertion_mode = InsertionMode::AfterHead;
                    }
                    return;
                }
                _ => {}
            }

            // "If last is true, then switch the insertion mode to "in body" and return."
            if last {
                self.insertion_mode = InsertionMode::InBody;
                return;
            }

            // "Let node now be the node before node in the stack of open elements."
            // (handled by loop decrement)
        }
    }

    /// [§ 13.2.6.2 Generate implied end tags](https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags)
    ///
    /// "When the steps below require the user agent to generate implied end tags,
    /// then, while the current node is a dd element, a dt element, an li element,
    /// an optgroup element, an option element, a p element, an rb element, an rp
    /// element, an rt element, or an rtc element, the user agent must pop the
    /// current node off the stack of open elements."
    fn generate_implied_end_tags(&mut self) {
        self.generate_implied_end_tags_excluding(None);
    }

    /// [§ 13.2.6.2 Generate implied end tags](https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags)
    ///
    /// "If a step requires the user agent to generate implied end tags but lists
    /// an element to exclude from the process, then the user agent must perform
    /// the above steps as if that element was not in the above list."
    fn generate_implied_end_tags_excluding(&mut self, exclude: Option<&str>) {
        const IMPLIED_END_TAG_ELEMENTS: &[&str] = &[
            "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
        ];

        while let Some(&current) = self.stack_of_open_elements.last() {
            if let Some(tag) = self.get_tag_name(current)
                && IMPLIED_END_TAG_ELEMENTS.contains(&tag)
                && exclude != Some(tag)
            {
                let _ = self.stack_of_open_elements.pop();
                continue;
            }
            break;
        }
    }

    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// This helper combines two spec operations commonly used together:
    ///
    /// [§ 13.2.6.2 Generate implied end tags](https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags)
    /// Then: Check if element is in scope and pop until found.
    ///
    /// Used for elements like <li>, <p>, <dd>, <dt> that implicitly close
    /// when a new one is encountered.
    fn close_element_if_in_scope(&mut self, tag_name: &str) {
        // STEP 1: Check if element is in scope.
        // Per spec, "p" uses button scope; others use default scope.
        // We must check scope BEFORE generating implied end tags, because
        // generate_implied_end_tags can pop elements (like <li>) that should
        // only be popped if the target element is actually in scope.
        let in_scope = if tag_name == "p" {
            self.has_element_in_button_scope(tag_name)
        } else {
            self.has_element_in_scope(tag_name)
        };

        if in_scope {
            // STEP 2: Generate implied end tags (excluding the target)
            self.generate_implied_end_tags_excluding(Some(tag_name));
            // STEP 3: Pop elements until target is found
            self.pop_until_tag(tag_name);
        }
    }
    /// [§ 13.2.4.3 Reconstruct the active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#reconstruct-the-active-formatting-elements)
    ///
    /// "When the steps below require the UA to reconstruct the active formatting
    /// elements, the UA must perform the following steps:"
    ///
    /// This algorithm has two phases:
    /// - Rewind phase (steps 4-6): Walk backwards to find where to start
    /// - Create phase (steps 7-10): Walk forwards, creating elements
    fn reconstruct_active_formatting_elements(&mut self) {
        // STEP 1: "If there are no entries in the list of active formatting
        //          elements, then there is nothing to reconstruct; stop this
        //          algorithm."
        if self.active_formatting_elements.is_empty() {
            return;
        }

        // STEP 2: "If the last (most recently added) entry in the list of active
        //          formatting elements is a marker, or if it is an element that
        //          is in the stack of open elements, then there is nothing to
        //          reconstruct; stop this algorithm."
        if let Some(last) = self.active_formatting_elements.last() {
            match last {
                ActiveFormattingElement::Marker => return,
                ActiveFormattingElement::Element { node_id, .. } => {
                    if self.stack_of_open_elements.contains(node_id) {
                        return;
                    }
                }
            }
        }

        // STEP 3: "Let entry be the last (most recently added) element in the
        //          list of active formatting elements."
        let mut entry_index = self.active_formatting_elements.len() - 1;

        // STEP 4-6: Rewind phase
        // "Rewind: If there are no entries before entry in the list of active
        //  formatting elements, then jump to the step labeled create."
        loop {
            // STEP 4: If at the beginning, jump to create (don't decrement)
            if entry_index == 0 {
                break;
            }

            // STEP 5: "Let entry be the entry one earlier than entry in the list
            //          of active formatting elements."
            entry_index -= 1;

            // STEP 6: "If entry is neither a marker nor an element that is also
            //          in the stack of open elements, go to the step labeled rewind."
            match &self.active_formatting_elements[entry_index] {
                ActiveFormattingElement::Marker => {
                    // Found marker, advance one position and start creating
                    entry_index += 1;
                    break;
                }
                ActiveFormattingElement::Element { node_id, .. } => {
                    if self.stack_of_open_elements.contains(node_id) {
                        // Found element in stack, advance one position and start creating
                        entry_index += 1;
                        break;
                    }
                    // Otherwise continue rewinding (implicit via loop)
                }
            }
        }

        // STEP 7-10: Create phase (advance and create loop)
        // "Advance: Let entry be the element one later than entry in the list
        //  of active formatting elements."
        loop {
            // STEP 8: "Create: Insert an HTML element for the token for which
            //          the element entry was created, to obtain new element."
            //
            // Clone the token first to avoid borrow checker issues
            let token = match &self.active_formatting_elements[entry_index] {
                ActiveFormattingElement::Element { token, .. } => token.clone(),
                ActiveFormattingElement::Marker => {
                    // Shouldn't happen after rewind, but handle gracefully
                    entry_index += 1;
                    if entry_index >= self.active_formatting_elements.len() {
                        break;
                    }
                    continue;
                }
            };

            let new_element_id = self.insert_html_element(&token);

            // STEP 9: "Replace the entry for entry in the list with an entry
            //          for new element."
            self.active_formatting_elements[entry_index] = ActiveFormattingElement::Element {
                node_id: new_element_id,
                token,
            };

            // STEP 10: "If the entry for new element in the list of active
            //           formatting elements is not the last entry in the list,
            //           return to the step labeled advance."
            entry_index += 1;
            if entry_index >= self.active_formatting_elements.len() {
                break;
            }
        }
    }
    /// [§ 13.2.4.3 The list of active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#push-onto-the-list-of-active-formatting-elements)
    ///
    /// "When the steps below require the UA to push onto the list of active
    /// formatting elements an element element, the UA must perform the
    /// following steps:"
    ///
    /// Includes the Noah's Ark clause: "If there are already three elements
    /// in the list of active formatting elements after the last marker...
    /// that have the same tag name, namespace, and attributes as element,
    /// then remove the earliest such element from the list."
    fn push_active_formatting_element(&mut self, node_id: NodeId, token: &Token) {
        // Noah's Ark clause:
        // STEP 1: Count matching elements after the last marker.
        if let Token::StartTag {
            name, attributes, ..
        } = token
        {
            let mut count = 0;
            let mut earliest_match_index = None;

            for (i, entry) in self.active_formatting_elements.iter().enumerate().rev() {
                match entry {
                    ActiveFormattingElement::Marker => break,
                    ActiveFormattingElement::Element {
                        token: entry_token, ..
                    } => {
                        if let Token::StartTag {
                            name: entry_name,
                            attributes: entry_attrs,
                            ..
                        } = entry_token
                            && entry_name == name
                            && entry_attrs == attributes
                        {
                            count += 1;
                            earliest_match_index = Some(i);
                        }
                    }
                }
            }

            // STEP 2: If 3 or more matches, remove the earliest.
            if count >= 3
                && let Some(idx) = earliest_match_index
            {
                let _ = self.active_formatting_elements.remove(idx);
            }
        }

        // STEP 3: Push the new entry.
        self.active_formatting_elements
            .push(ActiveFormattingElement::Element {
                node_id,
                token: token.clone(),
            });
    }

    /// [§ 13.2.4.3 Clear the list of active formatting elements up to the last marker](https://html.spec.whatwg.org/multipage/parsing.html#clear-the-list-of-active-formatting-elements-up-to-the-last-marker)
    ///
    /// "When the steps below require the UA to clear the list of active
    /// formatting elements up to the last marker, the UA must perform
    /// the following steps:
    ///
    /// 1. Let entry be the last (most recently added) entry in the list.
    /// 2. Remove entry from the list.
    /// 3. If entry was a marker, stop. Otherwise, go to step 1."
    fn clear_active_formatting_elements_to_last_marker(&mut self) {
        while let Some(entry) = self.active_formatting_elements.pop() {
            if matches!(entry, ActiveFormattingElement::Marker) {
                break;
            }
        }
    }

    /// [§ 13.1.1 Special](https://html.spec.whatwg.org/multipage/parsing.html#special)
    ///
    /// "The following elements have varying levels of special parsing rules:
    /// ... they are collectively known as special elements."
    fn is_special_element(tag_name: &str) -> bool {
        matches!(
            tag_name,
            "address"
                | "applet"
                | "area"
                | "article"
                | "aside"
                | "base"
                | "basefont"
                | "bgsound"
                | "blockquote"
                | "body"
                | "br"
                | "button"
                | "caption"
                | "center"
                | "col"
                | "colgroup"
                | "dd"
                | "details"
                | "dir"
                | "div"
                | "dl"
                | "dt"
                | "embed"
                | "fieldset"
                | "figcaption"
                | "figure"
                | "footer"
                | "form"
                | "frame"
                | "frameset"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "head"
                | "header"
                | "hgroup"
                | "hr"
                | "html"
                | "iframe"
                | "img"
                | "input"
                | "keygen"
                | "li"
                | "link"
                | "listing"
                | "main"
                | "marquee"
                | "menu"
                | "meta"
                | "nav"
                | "noembed"
                | "noframes"
                | "noscript"
                | "object"
                | "ol"
                | "p"
                | "param"
                | "plaintext"
                | "pre"
                | "script"
                | "search"
                | "section"
                | "select"
                | "source"
                | "style"
                | "summary"
                | "table"
                | "tbody"
                | "td"
                | "template"
                | "textarea"
                | "tfoot"
                | "th"
                | "thead"
                | "title"
                | "tr"
                | "track"
                | "ul"
                | "wbr"
                | "xmp"
        )
        // NOTE: MathML and SVG special elements omitted for now:
        // mi, mo, mn, ms, mtext, annotation-xml (MathML)
        // foreignObject, desc, title (SVG)
    }

    /// [§ 13.2.4.3 The list of active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#formatting)
    ///
    /// "The elements in the formatting category are: a, b, big, code, em, font,
    /// i, nobr, s, small, strike, strong, tt, u."
    #[allow(dead_code)]
    fn is_formatting_element(tag_name: &str) -> bool {
        matches!(
            tag_name,
            "a" | "b"
                | "big"
                | "code"
                | "em"
                | "font"
                | "i"
                | "nobr"
                | "s"
                | "small"
                | "strike"
                | "strong"
                | "tt"
                | "u"
        )
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#create-an-element-for-the-token)
    ///
    /// "Create an element for a token" — creates a DOM element from a token
    /// without inserting it into the tree or pushing onto the stack.
    /// Used by the adoption agency algorithm when creating replacement elements.
    fn create_element_for_token(&mut self, token: &Token) -> NodeId {
        if let Token::StartTag {
            name, attributes, ..
        } = token
        {
            self.create_element(Namespace::Html, name, attributes)
        } else {
            panic!("create_element_for_token called with non-StartTag token");
        }
    }

    /// [§ 13.2.6.4.7 "in body" - Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// 1. "Initialize node to be the current node (the bottommost node of the stack)."
    /// 2. "Loop: If node is an HTML element with the same tag name as the token, then:"
    ///    a. "Generate implied end tags, except for HTML elements with the same tag name
    ///    as the token."
    ///    b. "If node is not the current node, then this is a parse error."
    ///    c. "Pop all the nodes from the current node up to node, including node, then stop
    ///    these steps."
    /// 3. "Otherwise, if node is in the special category, then this is a parse error;
    ///    ignore the token, and return."
    /// 4. "Set node to the previous entry in the stack of open elements and return to
    ///    the step labeled loop."
    fn any_other_end_tag(&mut self, tag_name: &str) {
        // Walk the stack from top (current node) downward.
        let mut i = self.stack_of_open_elements.len();
        while i > 0 {
            i -= 1;
            let node_id = self.stack_of_open_elements[i];
            if let Some(node_tag) = self.get_tag_name(node_id) {
                // STEP 2: If node matches the tag name...
                if node_tag == tag_name {
                    // STEP 2a: Generate implied end tags, excluding same tag name.
                    self.generate_implied_end_tags_excluding(Some(tag_name));
                    // STEP 2c: Pop all nodes from current node up to and including node.
                    self.stack_of_open_elements.truncate(i);
                    return;
                }
                // STEP 3: If node is in the special category, ignore the token.
                if Self::is_special_element(node_tag) {
                    return;
                }
            }
            // STEP 4: Continue to previous entry.
        }
    }

    /// [§ 13.2.6.4.7 The adoption agency algorithm](https://html.spec.whatwg.org/multipage/parsing.html#adoption-agency-algorithm)
    ///
    /// "When the steps below require the UA to run the adoption agency algorithm
    /// for a token, the UA must perform the following steps:"
    fn run_adoption_agency(&mut self, tag_name: &str) {
        // STEP 1: "Let subject be token's tag name."
        let subject = tag_name;

        // STEP 2: "If the current node is an HTML element whose tag name is subject,
        //          and the current node is not in the list of active formatting elements,
        //          then pop the current node off the stack of open elements and return."
        if let Some(&current) = self.stack_of_open_elements.last()
            && self.get_tag_name(current) == Some(subject)
        {
            let in_afl = self.active_formatting_elements.iter().any(|e| {
                matches!(e, ActiveFormattingElement::Element { node_id, .. } if *node_id == current)
            });
            if !in_afl {
                let _ = self.stack_of_open_elements.pop();
                return;
            }
        }

        // STEP 3: "Let outer loop counter be 0."
        let mut outer_loop_counter = 0;

        // STEP 4: "Outer loop:"
        loop {
            // STEP 5: "If outer loop counter is greater than or equal to 8, then return."
            if outer_loop_counter >= 8 {
                return;
            }

            // STEP 6: "Increment outer loop counter by 1."
            outer_loop_counter += 1;

            // STEP 7: "Let formatting element be the last element in the list of active
            //          formatting elements that: is between the end of the list and the
            //          last marker in the list, if any, or the start of the list otherwise;
            //          and has the tag name subject."
            let formatting_element_afl_index = {
                let mut found = None;
                for (i, entry) in self.active_formatting_elements.iter().enumerate().rev() {
                    match entry {
                        ActiveFormattingElement::Marker => break,
                        ActiveFormattingElement::Element { token, .. } => {
                            if let Token::StartTag { name, .. } = token
                                && name == subject
                            {
                                found = Some(i);
                                break;
                            }
                        }
                    }
                }
                found
            };

            // STEP 8: "If there is no such element, then return and instead act as
            //          described in the 'any other end tag' entry above."
            let Some(formatting_element_afl_index) = formatting_element_afl_index else {
                self.any_other_end_tag(subject);
                return;
            };

            // Get the formatting element's NodeId
            let formatting_element_id =
                match &self.active_formatting_elements[formatting_element_afl_index] {
                    ActiveFormattingElement::Element { node_id, .. } => *node_id,
                    ActiveFormattingElement::Marker => unreachable!(),
                };

            // STEP 9: "If formatting element is not in the stack of open elements,
            //          then this is a parse error; remove the element from the list,
            //          and return."
            let formatting_element_stack_index = self
                .stack_of_open_elements
                .iter()
                .position(|&id| id == formatting_element_id);

            let Some(formatting_element_stack_index) = formatting_element_stack_index else {
                let _ = self
                    .active_formatting_elements
                    .remove(formatting_element_afl_index);
                return;
            };

            // STEP 10: "If formatting element is in the stack of open elements, but
            //           the element is not in scope, then this is a parse error; return."
            if !self.has_element_in_scope(subject) {
                return;
            }

            // STEP 11: "If formatting element is not the current node, this is a parse error."
            // (Continue regardless — parse error is informational.)

            // STEP 12: "Let furthest block be the topmost node in the stack of open
            //           elements that is lower than the formatting element in the stack
            //           and is an element in the special category."
            let furthest_block_index = {
                let mut found = None;
                for i in (formatting_element_stack_index + 1)..self.stack_of_open_elements.len() {
                    let node_id = self.stack_of_open_elements[i];
                    if let Some(tag) = self.get_tag_name(node_id)
                        && Self::is_special_element(tag)
                    {
                        found = Some(i);
                        break;
                    }
                }
                found
            };

            // STEP 13: "If there is no furthest block, then the UA must first pop all
            //           the nodes from the bottom of the stack of open elements, from the
            //           current node up to and including the formatting element, then remove
            //           the formatting element from the list of active formatting elements,
            //           and finally return."
            let Some(furthest_block_index) = furthest_block_index else {
                self.stack_of_open_elements
                    .truncate(formatting_element_stack_index);
                let _ = self
                    .active_formatting_elements
                    .remove(formatting_element_afl_index);
                return;
            };

            let furthest_block_id = self.stack_of_open_elements[furthest_block_index];

            // STEP 14: "Let common ancestor be the element immediately above the
            //           formatting element in the stack of open elements."
            let common_ancestor_id =
                self.stack_of_open_elements[formatting_element_stack_index - 1];

            // STEP 15: "Let a bookmark note the position of the formatting element
            //           in the list of active formatting elements relative to the
            //           elements on either side of it in the list."
            let mut bookmark = formatting_element_afl_index;

            // STEP 16: "Let node and last node be the furthest block."
            let mut node_stack_index = furthest_block_index;
            let mut last_node_id = furthest_block_id;

            // STEP 17: "Let inner loop counter be 0."
            let mut inner_loop_counter = 0;

            // STEP 18: "Inner loop:"
            loop {
                // STEP 18.1: "Increment inner loop counter by 1."
                inner_loop_counter += 1;

                // STEP 18.2: "Let node be the element immediately above node in the
                //             stack of open elements..."
                // We need to go UP the stack (toward index 0), but node_stack_index
                // might have shifted due to removals. We find the previous entry.
                node_stack_index -= 1;

                // "...or if node is no longer in the stack of open elements (e.g.,
                //  because it got removed by this algorithm), the element that was
                //  immediately above node in the stack of open elements before node
                //  was removed."
                let node_id = self.stack_of_open_elements[node_stack_index];

                // STEP 18.3: "If node is the formatting element, then break."
                if node_id == formatting_element_id {
                    break;
                }

                // STEP 18.4: "If inner loop counter is greater than 3 and node is in
                //             the list of active formatting elements, then remove node
                //             from the list."
                let node_afl_index = self
                    .active_formatting_elements
                    .iter()
                    .position(|e| matches!(e, ActiveFormattingElement::Element { node_id: nid, .. } if *nid == node_id));

                if inner_loop_counter > 3
                    && let Some(afl_idx) = node_afl_index
                {
                    let _ = self.active_formatting_elements.remove(afl_idx);
                    // Adjust bookmark if it was after the removed entry.
                    if bookmark > afl_idx {
                        bookmark -= 1;
                    }
                    // node_afl_index is now invalid; node is no longer in AFL.
                    // Fall through to step 18.5 which checks "not in AFL".
                }

                // Re-check node_afl_index after potential removal above.
                let node_afl_index = self
                    .active_formatting_elements
                    .iter()
                    .position(|e| matches!(e, ActiveFormattingElement::Element { node_id: nid, .. } if *nid == node_id));

                // STEP 18.5: "If node is not in the list of active formatting elements,
                //             then remove node from the stack of open elements and continue."
                if node_afl_index.is_none() {
                    let _ = self.stack_of_open_elements.remove(node_stack_index);
                    // node_stack_index now points to the next element (which was below node),
                    // but since we decrement at the top of the loop, we need to NOT decrement
                    // extra. Actually, the next iteration will decrement node_stack_index
                    // again. Since we removed the element at node_stack_index, the element
                    // that was at node_stack_index-1 is now still at node_stack_index-1,
                    // but node_stack_index is now pointing at the element that was below node.
                    // We need node_stack_index to be positioned so that decrementing gives
                    // us the element above the removed node. Since the element above was at
                    // node_stack_index-1 and is now at node_stack_index-1 (unchanged),
                    // we need to keep node_stack_index the same (it already points to what
                    // was below, and the loop will decrement to go above).
                    // Actually after remove, the elements shift down, so node_stack_index
                    // now points to what was below node. The element above the removed node
                    // is at node_stack_index - 1. Since the loop starts by decrementing,
                    // setting node_stack_index to node_stack_index (current) means next
                    // iteration will look at node_stack_index - 1 = element above removed.
                    // This is correct.
                    continue;
                }

                let node_afl_index = node_afl_index.unwrap();

                // STEP 18.6: "Create an element for the token for which the element
                //             node was created, in the HTML namespace, with common
                //             ancestor as the intended parent; replace the entry for
                //             node in the list of active formatting elements with an
                //             entry for the new element, replace the entry for node
                //             in the stack of open elements with an entry for the new
                //             element, and let node be the new element."
                let node_token = match &self.active_formatting_elements[node_afl_index] {
                    ActiveFormattingElement::Element { token, .. } => token.clone(),
                    ActiveFormattingElement::Marker => unreachable!(),
                };
                let new_element_id = self.create_element_for_token(&node_token);

                // Replace in AFL
                self.active_formatting_elements[node_afl_index] =
                    ActiveFormattingElement::Element {
                        node_id: new_element_id,
                        token: node_token,
                    };

                // Replace in stack
                self.stack_of_open_elements[node_stack_index] = new_element_id;

                let node_id = new_element_id;

                // STEP 18.7: "If last node is the furthest block, then move the
                //             bookmark to be immediately after the new node in the
                //             list of active formatting elements."
                if last_node_id == furthest_block_id {
                    bookmark = node_afl_index + 1;
                }

                // STEP 18.8: "Append last node to node."
                // First remove last_node from its current parent.
                if let Some(parent) = self.tree.parent(last_node_id) {
                    self.tree.remove_child(parent, last_node_id);
                }
                self.tree.append_child(node_id, last_node_id);

                // STEP 18.9: "Set last node to node."
                last_node_id = node_id;
            }

            // STEP 19: "Insert whatever last node ended up being in the previous step
            //           at the appropriate place for inserting a node, but using common
            //           ancestor as the override target."
            // Remove last_node from its current parent first.
            if let Some(parent) = self.tree.parent(last_node_id) {
                self.tree.remove_child(parent, last_node_id);
            }
            self.tree.append_child(common_ancestor_id, last_node_id);

            // STEP 20: "Create an element for the token for which the formatting
            //           element was created, in the HTML namespace, with the furthest
            //           block as the intended parent."
            let formatting_token =
                match &self.active_formatting_elements[formatting_element_afl_index] {
                    ActiveFormattingElement::Element { token, .. } => token.clone(),
                    ActiveFormattingElement::Marker => unreachable!(),
                };
            let new_element_id = self.create_element_for_token(&formatting_token);

            // STEP 21: "Take all of the child nodes of the furthest block and append
            //           them to the new element created in the previous step."
            self.tree.move_children(furthest_block_id, new_element_id);

            // STEP 22: "Append that new element to the furthest block."
            self.tree.append_child(furthest_block_id, new_element_id);

            // STEP 23: "Remove the formatting element from the list of active formatting
            //           elements, and insert the new element into the list of active
            //           formatting elements at the position of the aforementioned bookmark."
            // First remove old entry. Adjust bookmark if needed.
            let _ = self
                .active_formatting_elements
                .remove(formatting_element_afl_index);
            if bookmark > formatting_element_afl_index {
                bookmark -= 1;
            }
            // Clamp bookmark to valid range.
            if bookmark > self.active_formatting_elements.len() {
                bookmark = self.active_formatting_elements.len();
            }
            self.active_formatting_elements.insert(
                bookmark,
                ActiveFormattingElement::Element {
                    node_id: new_element_id,
                    token: formatting_token,
                },
            );

            // STEP 24: "Remove the formatting element from the stack of open elements,
            //           and insert the new element into the stack of open elements
            //           immediately below the position of the furthest block in that stack."
            // Remove old formatting element from stack.
            if let Some(pos) = self
                .stack_of_open_elements
                .iter()
                .position(|&id| id == formatting_element_id)
            {
                let _ = self.stack_of_open_elements.remove(pos);
            }
            // Find furthest block position (may have shifted after removal).
            if let Some(fb_pos) = self
                .stack_of_open_elements
                .iter()
                .position(|&id| id == furthest_block_id)
            {
                self.stack_of_open_elements
                    .insert(fb_pos + 1, new_element_id);
            }
        }
    }

    /// [§ 13.2.6.4.1 The "initial" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode)
    fn handle_initial_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Ignore the token."
            Token::Character { data } if Self::is_whitespace(*data) => {}

            // "A comment token"
            // "Insert a comment as the last child of the Document object."
            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            // "A DOCTYPE token"
            // "If the DOCTYPE token's name is not "html", or the token's public identifier is not
            // missing, or the token's system identifier is neither missing nor "about:legacy-compat",
            // then there is a parse error."
            // ...
            // "Then, switch the insertion mode to "before html"."
            Token::Doctype { .. } => {
                // NOTE: We skip creating a DocumentType node for simplicity.
                // The full spec requires appending a DocumentType node to the Document.
                self.insertion_mode = InsertionMode::BeforeHtml;
            }

            // "Anything else"
            // "If the document is not an iframe srcdoc document, then this is a parse error;
            // if the parser cannot change the mode flag is false, set the Document to quirks mode."
            // "In any case, switch the insertion mode to "before html", then reprocess the token."
            _ => {
                self.insertion_mode = InsertionMode::BeforeHtml;
                self.reprocess_token(token);
            }
        }
    }

    /// [§ 13.2.6.4.2 The "before html" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode)
    fn handle_before_html_mode(&mut self, token: &Token) {
        match token {
            // "An end tag whose tag name is one of: "head", "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "head" | "body" | "html" | "br") =>
            {
                self.handle_before_html_anything_else(token);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            //
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } | Token::EndTag { .. } => {}

            // "A comment token"
            // "Insert a comment as the last child of the Document object."
            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Ignore the token."
            Token::Character { data } if Self::is_whitespace(*data) => {}

            // "A start tag whose tag name is "html""
            // "Create an element for the token in the HTML namespace, with the Document as the
            // intended parent. Append it to the Document object. Put this element in the stack
            // of open elements."
            // ...
            // "Switch the insertion mode to "before head"."
            Token::StartTag {
                name, attributes, ..
            } if name == "html" => {
                let html_idx = self.create_element(Namespace::Html, name, attributes);
                self.append_child(NodeId::ROOT, html_idx);
                self.stack_of_open_elements.push(html_idx);
                self.insertion_mode = InsertionMode::BeforeHead;
            }

            // "Anything else"
            _ => {
                self.handle_before_html_anything_else(token);
            }
        }
    }

    /// [§ 13.2.6.4.2 The "before html" insertion mode - Anything else](https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode)
    ///
    /// "Anything else":
    /// "Create an html element whose node document is the Document object.
    /// Append it to the Document object. Put this element in the stack of
    /// open elements. Switch the insertion mode to "before head", then
    /// reprocess the token."
    fn handle_before_html_anything_else(&mut self, token: &Token) {
        // STEP 1: "Create an html element whose node document is the Document object."
        let html_idx = self.create_element(Namespace::Html, "html", &[]);

        // STEP 2: "Append it to the Document object."
        self.append_child(NodeId::ROOT, html_idx);

        // STEP 3: "Put this element in the stack of open elements."
        self.stack_of_open_elements.push(html_idx);

        // STEP 4: "Switch the insertion mode to 'before head'."
        self.insertion_mode = InsertionMode::BeforeHead;

        // STEP 5: "Reprocess the token."
        self.reprocess_token(token);
    }

    /// [§ 13.2.6.4.3 The "before head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode)
    fn handle_before_head_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Ignore the token."
            Token::Character { data } if Self::is_whitespace(*data) => {}

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "A start tag whose tag name is "html""
            // "Process the token using the rules for the "in body" insertion mode."
            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // "A start tag whose tag name is "head""
            // "Insert an HTML element for the token."
            // "Set the head element pointer to the newly created head element."
            // "Switch the insertion mode to "in head"."
            Token::StartTag { name, .. } if name == "head" => {
                let head_idx = self.insert_html_element(token);
                self.head_element_pointer = Some(head_idx);
                self.insertion_mode = InsertionMode::InHead;
            }

            // "An end tag whose tag name is one of: "head", "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "head" | "body" | "html" | "br") =>
            {
                self.handle_before_head_anything_else(token);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            //
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } | Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_before_head_anything_else(token);
            }
        }
    }

    /// [§ 13.2.6.4.3 The "before head" insertion mode - Anything else](https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode)
    ///
    /// "Anything else":
    /// "Insert an HTML element for a "head" start tag token with no attributes.
    /// Set the head element pointer to the newly created head element.
    /// Switch the insertion mode to "in head". Reprocess the current token."
    fn handle_before_head_anything_else(&mut self, token: &Token) {
        // STEP 1: "Insert an HTML element for a 'head' start tag token with no attributes."
        let head_idx = self.create_element(Namespace::Html, "head", &[]);
        let parent_idx = self.current_node().unwrap_or(NodeId::ROOT);
        self.append_child(parent_idx, head_idx);
        self.stack_of_open_elements.push(head_idx);

        // STEP 2: "Set the head element pointer to the newly created head element."
        self.head_element_pointer = Some(head_idx);

        // STEP 3: "Switch the insertion mode to 'in head'."
        self.insertion_mode = InsertionMode::InHead;

        // STEP 4: "Reprocess the current token."
        self.reprocess_token(token);
    }

    /// [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
    fn handle_in_head_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Insert the character."
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.insert_character(*data);
            }

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "An end tag whose tag name is "head""
            // "Pop the current node (which will be the head element) off the stack of open elements."
            // "Switch the insertion mode to "after head"."
            Token::EndTag { name, .. } if name == "head" => {
                let _ = self.stack_of_open_elements.pop();
                self.insertion_mode = InsertionMode::AfterHead;
            }

            // "An end tag whose tag name is one of: "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. } if matches!(name.as_str(), "body" | "html" | "br") => {
                self.handle_in_head_anything_else(token);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            //
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } | Token::EndTag { .. } => {}

            // "A start tag whose tag name is "html""
            // "Process the token using the rules for the "in body" insertion mode."
            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // "A start tag whose tag name is one of: "base", "basefont", "bgsound", "link""
            // "Insert an HTML element for the token. Immediately pop the current node off the
            // stack of open elements."
            // "Acknowledge the token's self-closing flag, if it is set."
            //
            // "A start tag whose tag name is "meta""
            // "Insert an HTML element for the token. Immediately pop the current node off the
            // stack of open elements."
            // ...
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "base" | "basefont" | "bgsound" | "link" | "meta"
                ) =>
            {
                let _ = self.insert_html_element(token);
                let _ = self.stack_of_open_elements.pop();
            }

            // "A start tag whose tag name is "title""
            // "Follow the generic RCDATA element parsing algorithm."
            //
            // [§ 13.2.6.2 The generic RCDATA element parsing algorithm](https://html.spec.whatwg.org/multipage/parsing.html#generic-rcdata-element-parsing-algorithm):
            // 1. "Insert an HTML element for the token."
            // 2. "If the parser was created as part of the HTML fragment parsing algorithm, then
            //     mark the script element as "already started"." (N/A)
            // 3. "Let the original insertion mode be the current insertion mode."
            // 4. "Switch the insertion mode to "text"."
            Token::StartTag { name, .. } if name == "title" => {
                let _ = self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The spec also says "Switch the tokenizer to the RCDATA state."
                // We don't have tokenizer integration, so we rely on the tokenizer
                // emitting character tokens that the Text mode will handle.
            }

            // "A start tag whose tag name is one of: "noscript", "noframes", "style""
            // "Follow the generic raw text element parsing algorithm."
            //
            // [§ 13.2.6.3 The generic raw text element parsing algorithm](https://html.spec.whatwg.org/multipage/parsing.html#generic-raw-text-element-parsing-algorithm):
            // 1. "Insert an HTML element for the token."
            // 2. "Let the original insertion mode be the current insertion mode."
            // 3. "Switch the insertion mode to "text"."
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "style" | "noscript" | "noframes") =>
            {
                let _ = self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The tokenizer handles switching to RAWTEXT state for these elements
            }

            // [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
            // "A start tag whose tag name is "script""
            // "Run these steps:"
            // 1-8. (Simplified) Insert an HTML element for the token.
            // 9. "Switch the tokenizer to the script data state."
            // 10. "Let the original insertion mode be the current insertion mode."
            // 11. "Switch the insertion mode to "text"."
            Token::StartTag { name, .. } if name == "script" => {
                let _ = self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The tokenizer handles switching to ScriptData state for script elements
            }

            // [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
            // "A start tag whose tag name is "template""
            //
            // Full spec requires:
            // 1. Insert an HTML element for the token.
            // 2. Insert a marker at the end of the list of active formatting elements.
            // 3. Set the frameset-ok flag to "not ok".
            // 4. Switch the insertion mode to "in template".
            // 5. Push "in template" onto the stack of template insertion modes.
            //
            // NOTE: InTemplate mode is not yet implemented. For now, we insert the element
            // and stay in current mode to avoid an infinite reprocessing loop. Template
            // content will be parsed as regular HTML content (incorrect per spec, but
            // prevents stack overflow).
            Token::StartTag { name, .. } if name == "template" => {
                let _ = self.insert_html_element(token);
                // TODO: Implement full template handling with InTemplate mode
            }

            // "Anything else"
            _ => {
                self.handle_in_head_anything_else(token);
            }
        }
    }

    /// [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
    ///
    /// "Anything else":
    /// "Pop the current node (which will be the head element) off the stack of open elements."
    /// "Switch the insertion mode to "after head"."
    /// "Reprocess the token."
    fn handle_in_head_anything_else(&mut self, token: &Token) {
        // STEP 1: "Pop the current node (which will be the head element)
        // off the stack of open elements."
        let _ = self.stack_of_open_elements.pop();

        // STEP 2: "Switch the insertion mode to "after head"."
        self.insertion_mode = InsertionMode::AfterHead;

        // STEP 3: "Reprocess the token."
        self.reprocess_token(token);
    }
    fn handle_in_head_noscript_mode(&mut self, token: &Token) {
        match token {
            // A start tag whose tag name is "html"
            Token::StartTag { name, .. } if name == "html" => {
                // Process the token using the rules for the "in body" insertion mode.
                self.handle_in_body_mode(token);
            }
            // "An end tag whose tag name is "noscript""
            // "Pop the current node (which will be a noscript element) from the stack of
            //  open elements; the new current node will be a head element."
            // "Switch the insertion mode to "in head"."
            Token::EndTag { name, .. } if name == "noscript" => {
                let _ = self.stack_of_open_elements.pop();
                self.insertion_mode = InsertionMode::InHead;
            }

            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            //  U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Process the token using the rules for the "in head" insertion mode."
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.handle_in_head_mode(token);
            }

            // "A comment token"
            // "Process the token using the rules for the "in head" insertion mode."
            Token::Comment { .. } => {
                self.handle_in_head_mode(token);
            }

            // "A start tag whose tag name is one of: "basefont", "bgsound", "link", "meta",
            //  "noframes", "style""
            // "Process the token using the rules for the "in head" insertion mode."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "basefont" | "bgsound" | "link" | "meta" | "noframes" | "style"
                ) =>
            {
                self.handle_in_head_mode(token);
            }

            // "An end tag whose tag name is "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. } if name == "br" => {
                // Pop the current node (which will be a noscript element) from the stack of open elements.
                let _ = self.stack_of_open_elements.pop();
                // Switch the insertion mode to "in head".
                self.insertion_mode = InsertionMode::InHead;
                // Reprocess the token.
                self.reprocess_token(token);
            }

            // "A start tag whose tag name is one of: "head", "noscript""
            // "Parse error. Ignore the token."
            Token::StartTag { name, .. } if matches!(name.as_str(), "head" | "noscript") => {
                // Parse error. Ignore the token.
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            //
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } | Token::EndTag { .. } => {
                // Parse error. Ignore the token.
            }

            // Anything else
            _ => {
                // TODO: Parse error.

                // Pop the current node (which will be a noscript element) from the stack of open elements; the new current node will be a head element.
                let _ = self.stack_of_open_elements.pop();
                // Switch the insertion mode to "in head".
                self.insertion_mode = InsertionMode::InHead;
                // Reprocess the token.
                self.reprocess_token(token);
            }
        }
    }
    /// [§ 13.2.6.4.8 The "text" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incdata)
    fn handle_text_mode(&mut self, token: &Token) {
        match token {
            // "A character token"
            // "Insert the character."
            Token::Character { data } => {
                self.insert_character(*data);
            }

            // "An end-of-file token"
            // "Parse error."
            // "If the current node is a script element, then set its already started to true."
            // "Pop the current node off the stack of open elements."
            // "Switch the insertion mode to the original insertion mode and reprocess the token."
            Token::EndOfFile => {
                // Parse error (logged implicitly)
                let _ = self.stack_of_open_elements.pop();
                self.insertion_mode = self
                    .original_insertion_mode
                    .unwrap_or(InsertionMode::InBody);
                // NOTE: Spec says to reprocess, but EOF is terminal so we just switch mode.
            }

            // "An end tag whose tag name is "script""
            // (Complex script handling - not implemented)
            //
            // "Any other end tag"
            // "Pop the current node off the stack of open elements."
            // "Switch the insertion mode to the original insertion mode."
            Token::EndTag { .. } => {
                let _ = self.stack_of_open_elements.pop();
                self.insertion_mode = self
                    .original_insertion_mode
                    .unwrap_or(InsertionMode::InBody);
            }

            // NOTE: Start tags and other tokens should not appear in text mode
            // per the tokenizer's behavior. If they do, it indicates a bug.
            _ => {
                panic!(
                    "Unexpected token in Text mode: {token:?}. This indicates a tokenizer or parser bug."
                );
            }
        }
    }

    /// [§ 13.2.6.4.6 The "after head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode)
    fn handle_after_head_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Insert the character."
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.insert_character(*data);
            }

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "A start tag whose tag name is "html""
            // "Process the token using the rules for the "in body" insertion mode."
            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // "A start tag whose tag name is "body""
            // "Insert an HTML element for the token."
            // "Set the frameset-ok flag to "not ok"."
            // "Switch the insertion mode to "in body"."
            Token::StartTag { name, .. } if name == "body" => {
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InBody;
            }

            // "A start tag whose tag name is "head""
            // "Parse error. Ignore the token."
            Token::StartTag { name, .. } if name == "head" => {}

            // "An end tag whose tag name is one of: "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. } if matches!(name.as_str(), "body" | "html" | "br") => {
                self.handle_after_head_anything_else(token);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            //
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } | Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_after_head_anything_else(token);
            }
        }
    }

    /// [§ 13.2.6.4.6 The "after head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode)
    ///
    /// "Anything else":
    /// "Insert an HTML element for a "body" start tag token with no attributes."
    /// "Switch the insertion mode to "in body"."
    /// "Reprocess the current token."
    fn handle_after_head_anything_else(&mut self, token: &Token) {
        // STEP 1: "Insert an HTML element for a "body" start tag token with
        // no attributes."
        //
        // [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#insert-an-html-element)
        // We manually create the body element and insert it, since we don't
        // have a real "body" start tag token.
        let body_idx = self.create_element(Namespace::Html, "body", &[]);
        let parent_idx = self.current_node().unwrap_or(NodeId::ROOT);
        self.append_child(parent_idx, body_idx);
        self.stack_of_open_elements.push(body_idx);

        // STEP 2: "Switch the insertion mode to "in body"."
        self.insertion_mode = InsertionMode::InBody;

        // STEP 3: "Reprocess the current token."
        self.reprocess_token(token);
    }

    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// This is the main tree construction mode for document content. The spec
    /// organizes token handling as follows:
    ///
    /// - Character tokens (NULL, whitespace, other)
    /// - Comment tokens
    /// - DOCTYPE tokens (parse error, ignore)
    /// - Start tag tokens (html, base/link/meta, head, body, frameset, formatting
    ///   elements, block elements, void elements, etc.)
    /// - End tag tokens (body, html, block elements, formatting elements, etc.)
    /// - End-of-file token
    ///
    /// ## Implemented:
    /// - Block-level start/end tags (div, p, headings, lists, etc.)
    /// - Void elements (br, hr, img, etc.)
    /// - Character and comment insertion
    /// - Basic formatting tags (b, i, strong, em, etc.)
    ///
    /// ## Not Implemented:
    /// - [§ 13.2.4.3] List of active formatting elements
    /// - [§ 13.2.6.4.7] Adoption agency algorithm (for misnested formatting)
    /// - [§ 13.2.6.1] Foster parenting (for table content errors)
    /// - Form element pointer
    /// - Frameset handling
    /// - Template element handling
    fn handle_in_body_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is U+0000 NULL"
            // "Parse error. Ignore the token."
            //
            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Character { data: '\0' } | Token::Doctype { .. } => {}

            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Reconstruct the active formatting elements, if any."
            // "Insert the character."
            //
            // "Any other character token"
            // "Reconstruct the active formatting elements, if any."
            // "Insert the character."
            // "Set the frameset-ok flag to "not ok"."
            Token::Character { data } => {
                self.reconstruct_active_formatting_elements();
                self.insert_character(*data);
                // TODO: Set the frameset-ok flag to "not ok
            }

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "A start tag whose tag name is "html""
            // "Parse error."
            // "If there is a template element on the stack of open elements, then ignore the token."
            // "Otherwise, for each attribute on the token, check to see if the attribute is already
            // present on the top element of the stack of open elements. If it is not, add the
            // attribute and its corresponding value to that element."
            Token::StartTag { name, .. } if name == "html" => {
                // Parse error. Simplified: ignore attribute merging.
            }

            // "A start tag whose tag name is one of: "address", "article", "aside", "blockquote",
            // "center", "details", "dialog", "dir", "div", "dl", "fieldset", "figcaption", "figure",
            // "footer", "header", "hgroup", "main", "menu", "nav", "ol", "p", "search", "section",
            // "summary", "ul""
            // "If the stack of open elements has a p element in button scope, then close a p element."
            // "Insert an HTML element for the token."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "address"
                        | "article"
                        | "aside"
                        | "blockquote"
                        | "center"
                        | "details"
                        | "dialog"
                        | "dir"
                        | "div"
                        | "dl"
                        | "fieldset"
                        | "figcaption"
                        | "figure"
                        | "footer"
                        | "header"
                        | "hgroup"
                        | "main"
                        | "menu"
                        | "nav"
                        | "ol"
                        | "search"
                        | "section"
                        | "summary"
                        | "ul"
                ) =>
            {
                // "If the stack of open elements has a p element in button scope, then close a p element."
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "p"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "p""
            // "If the stack of open elements has a p element in button scope, then close a p element."
            // "Insert an HTML element for the token."
            Token::StartTag { name, .. } if name == "p" => {
                // Close any existing <p> element first
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "form"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "form""
            // "If the form element pointer is not null, and there is no template element on the
            //  stack of open elements, then this is a parse error; ignore the token."
            // "Otherwise:"
            // "If the stack of open elements has a p element in button scope, then close a p element."
            // "Insert an HTML element for the token, and, if there is no template element on the
            //  stack of open elements, set the form element pointer to point to the element created."
            Token::StartTag { name, .. } if name == "form" => {
                // NOTE: Simplified - we skip form element pointer tracking
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "pre", "listing"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is one of: "pre", "listing""
            Token::StartTag { name, .. } if matches!(name.as_str(), "pre" | "listing") => {
                // STEP 1: Close any p element in button scope.
                // "If the stack of open elements has a p element in button scope, then close a p element."
                self.close_element_if_in_scope("p");

                // STEP 2: Insert the element.
                // "Insert an HTML element for the token."
                let _ = self.insert_html_element(token);

                // STEP 3: Skip leading newline.
                // "If the next token is a U+000A LINE FEED (LF) character token, then ignore that
                //  token and move on to the next one. (Newlines at the start of pre blocks are
                //  ignored as an authoring convenience.)"
                //
                // NOTE: This requires peeking at the next token, which our current architecture
                // doesn't support. The tokenizer would need to expose a peek method, or we'd need
                // to track state to skip the next LF in process_token.
                // TODO: Implement LF skipping for pre/listing start tags

                // STEP 4: Set frameset-ok flag.
                // "Set the frameset-ok flag to "not ok"."
                // TODO: self.frameset_ok = false;
            }

            // [§ 13.2.6.4.7 "in body" - Start tag h1-h6](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "h1", "h2", "h3", "h4", "h5", "h6""
            // "If the stack of open elements has a p element in button scope, then close a p element."
            // "If the current node is an HTML element whose tag name is one of "h1", "h2", "h3",
            //  "h4", "h5", or "h6", then this is a parse error; pop the current node off the stack
            //  of open elements."
            // "Insert an HTML element for the token."
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") =>
            {
                self.close_element_if_in_scope("p");
                // If currently in a heading, close it (headings don't nest)
                if let Some(idx) = self.current_node()
                    && let Some(tag) = self.get_tag_name(idx)
                    && matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
                {
                    let _ = self.stack_of_open_elements.pop();
                }
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "a"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is "a""
            // "If the list of active formatting elements contains an a element between the end
            //  of the list and the last marker on the list (or the start of the list if there
            //  is no marker on the list), then this is a parse error; run the adoption agency
            //  algorithm for the token 'a', then remove that element from the list of active
            //  formatting elements and the stack of open elements if the adoption agency
            //  algorithm didn't already remove it."
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Push onto the list of active formatting elements that element."
            Token::StartTag { name, .. } if name == "a" => {
                // STEP 1: Check for existing <a> in AFL (after last marker).
                let existing_a = self
                    .active_formatting_elements
                    .iter()
                    .rev()
                    .take_while(|e| !matches!(e, ActiveFormattingElement::Marker))
                    .find_map(|e| {
                        if let ActiveFormattingElement::Element { node_id, token } = e
                            && let Token::StartTag { name, .. } = token
                            && name == "a"
                        {
                            return Some(*node_id);
                        }
                        None
                    });

                if let Some(existing_a_id) = existing_a {
                    // Run adoption agency for "a"
                    self.run_adoption_agency("a");
                    // Remove from AFL if still there
                    self.active_formatting_elements.retain(|e| {
                        !matches!(e, ActiveFormattingElement::Element { node_id, .. } if *node_id == existing_a_id)
                    });
                    // Remove from stack if still there
                    self.stack_of_open_elements
                        .retain(|&id| id != existing_a_id);
                }

                // STEP 2: Reconstruct and insert.
                self.reconstruct_active_formatting_elements();
                let element_id = self.insert_html_element(token);
                self.push_active_formatting_element(element_id, token);
            }

            // [§ 13.2.6.4.7 "in body" - Formatting element start tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is one of: "b", "big", "code", "em", "font", "i",
            //  "s", "small", "strike", "strong", "tt", "u""
            //
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Push onto the list of active formatting elements that element."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "b" | "big"
                        | "code"
                        | "em"
                        | "font"
                        | "i"
                        | "s"
                        | "small"
                        | "strike"
                        | "strong"
                        | "tt"
                        | "u"
                        | "nobr"
                ) =>
            {
                self.reconstruct_active_formatting_elements();
                let element_id = self.insert_html_element(token);
                self.push_active_formatting_element(element_id, token);
            }

            // [§ 13.2.6.4.7 "in body" - Other inline start tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // These are NOT formatting elements per § 13.2.4.3, so they are not pushed
            // onto the active formatting elements list. They still reconstruct.
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "span"
                        | "label"
                        | "abbr"
                        | "cite"
                        | "dfn"
                        | "kbd"
                        | "mark"
                        | "q"
                        | "ruby"
                        | "samp"
                        | "sub"
                        | "sup"
                        | "time"
                        | "var"
                        | "bdi"
                        | "bdo"
                        | "data"
                ) =>
            {
                self.reconstruct_active_formatting_elements();
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "li"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "li""
            // "Run these steps:
            //  1. Set the frameset-ok flag to "not ok".
            //  2. Initialize node to be the current node (the bottommost node of the stack).
            //  3. Loop: If node is an li element, then run these substeps:
            //     - Generate implied end tags, except for li elements.
            //     - If the current node is not an li element, then this is a parse error.
            //     - Pop elements from the stack of open elements until an li element has been popped.
            //     - Jump to the step labeled done below.
            //  4. If node is in the special category, but is not an address, div, or p element,
            //     then jump to the step labeled done below.
            //  5. Otherwise, set node to the previous entry in the stack of open elements and
            //     return to the step labeled loop.
            //  ...
            //  8. Done: If the stack of open elements has a p element in button scope, then close a p element.
            //  9. Insert an HTML element for the token."
            Token::StartTag { name, .. } if name == "li" => {
                // TODO: STEP 1: Set frameset-ok flag to "not ok".

                // STEP 2-5: Walk the stack backwards looking for <li>.
                let mut found_li = false;
                for i in (0..self.stack_of_open_elements.len()).rev() {
                    let node_id = self.stack_of_open_elements[i];
                    if let Some(tag) = self.get_tag_name(node_id) {
                        // STEP 3: If node is "li", close it.
                        if tag == "li" {
                            found_li = true;
                            break;
                        }
                        // STEP 4: If node is special but not address/div/p, stop.
                        if Self::is_special_element(tag) && !matches!(tag, "address" | "div" | "p")
                        {
                            break;
                        }
                        // STEP 5: Otherwise continue to previous entry.
                    }
                }
                if found_li {
                    self.generate_implied_end_tags_excluding(Some("li"));
                    // If current node is not "li", this is a parse error (ignored).
                    self.pop_until_tag("li");
                }

                // STEP 8: Done. If <p> in button scope, close it.
                if self.has_element_in_button_scope("p") {
                    self.close_element_if_in_scope("p");
                }

                // STEP 9: Insert an HTML element for the token.
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "dd", "dt"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "dd", "dt""
            // Same pattern as <li> but the loop checks for both "dd" and "dt".
            //  1. Set the frameset-ok flag to "not ok".
            //  2. Initialize node to be the current node.
            //  3. Loop:
            //     - If node is "dd": generate implied end tags excluding "dd",
            //       pop until "dd", jump to Done.
            //     - If node is "dt": generate implied end tags excluding "dt",
            //       pop until "dt", jump to Done.
            //     - If node is special but not address/div/p: break.
            //     - Otherwise: set node to previous entry, continue.
            //  8. Done: If <p> in button scope, close <p>.
            //  9. Insert an HTML element for the token.
            Token::StartTag { name, .. } if matches!(name.as_str(), "dd" | "dt") => {
                // TODO: STEP 1: Set frameset-ok flag to "not ok".

                // STEP 2-5: Walk the stack backwards looking for "dd" or "dt".
                let mut found_tag: Option<&str> = None;
                for i in (0..self.stack_of_open_elements.len()).rev() {
                    let node_id = self.stack_of_open_elements[i];
                    if let Some(tag) = self.get_tag_name(node_id) {
                        if tag == "dd" {
                            found_tag = Some("dd");
                            break;
                        }
                        if tag == "dt" {
                            found_tag = Some("dt");
                            break;
                        }
                        if Self::is_special_element(tag) && !matches!(tag, "address" | "div" | "p")
                        {
                            break;
                        }
                    }
                }
                if let Some(close_tag) = found_tag {
                    self.generate_implied_end_tags_excluding(Some(close_tag));
                    self.pop_until_tag(close_tag);
                }

                // STEP 8: Done. If <p> in button scope, close it.
                if self.has_element_in_button_scope("p") {
                    self.close_element_if_in_scope("p");
                }

                // STEP 9: Insert an HTML element for the token.
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "button"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is "button""
            Token::StartTag { name, .. } if name == "button" => {
                // STEP 1: Close any existing button in scope.
                if self.has_element_in_scope("button") {
                    // "If the stack of open elements has a button element in scope, then this is a
                    //  parse error; run these substeps:
                    //    TODO: 1. Generate implied end tags.
                    //    2. Pop elements from the stack of open elements until a button element
                    //       has been popped from the stack."
                    self.pop_until_tag("button");
                }
                // STEP 2: Reconstruct active formatting elements.
                // "Reconstruct the active formatting elements, if any."
                self.reconstruct_active_formatting_elements();

                // STEP 3: Insert the button element.
                // "Insert an HTML element for the token."
                let _ = self.insert_html_element(token);

                // STEP 4: Set frameset-ok flag.
                // TODO: Set the frameset-ok flag to "not ok".
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "applet", "marquee", "object"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "applet", "marquee", "object""
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Insert a marker at the end of the list of active formatting elements."
            // "Set the frameset-ok flag to "not ok"."
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "applet" | "marquee" | "object") =>
            {
                self.reconstruct_active_formatting_elements();
                let _ = self.insert_html_element(token);
                // "Insert a marker at the end of the list of active formatting elements."
                self.active_formatting_elements
                    .push(ActiveFormattingElement::Marker);
                // TODO: Set the frameset-ok flag to "not ok".
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "select"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is "select""
            Token::StartTag { name, .. } if name == "select" => {
                // STEP 1: Reconstruct the active formatting elements.
                // "Reconstruct the active formatting elements, if any."
                self.reconstruct_active_formatting_elements();

                // STEP 2: Insert the select element.
                // "Insert an HTML element for the token."
                let _ = self.insert_html_element(token);

                // TODO: STEP 3: Set the frameset-ok flag.
                // "Set the frameset-ok flag to "not ok"."
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "optgroup", "option"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "optgroup", "option""
            // "If the current node is an option element, then pop the current node off the stack."
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            Token::StartTag { name, .. } if matches!(name.as_str(), "optgroup" | "option") => {
                // Close current option if any
                if let Some(&node_id) = self.stack_of_open_elements.last()
                    && self.get_tag_name(node_id) == Some("option")
                {
                    let _ = self.stack_of_open_elements.pop();
                }
                self.reconstruct_active_formatting_elements();
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "iframe"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "iframe""
            // "Set the frameset-ok flag to "not ok"."
            // "Follow the generic raw text element parsing algorithm."
            Token::StartTag { name, .. } if name == "iframe" => {
                let _ = self.insert_html_element(token);
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "textarea"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "textarea""
            // "Insert an HTML element for the token."
            // "If the next token is a U+000A LINE FEED (LF) character token, then ignore that token."
            // "Switch the tokenizer to the RCDATA state."
            // "Let the original insertion mode be the current insertion mode."
            // "Set the frameset-ok flag to "not ok"."
            // "Switch the insertion mode to "text"."
            // NOTE: Tokenizer state switching handled by tokenizer based on tag name.
            Token::StartTag { name, .. } if name == "textarea" => {
                let _ = self.insert_html_element(token);
                // TODO: Skip next LF if present
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
            }

            // [§ 13.2.6.4.7 "in body" - End tag "li"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "An end tag whose tag name is "li""
            // "If the stack of open elements does not have an li element in
            //  list item scope, then this is a parse error; ignore the token."
            // "Otherwise, run these steps:
            //  1. Generate implied end tags, except for li elements.
            //  2. If the current node is not an li element, then this is a parse error.
            //  3. Pop elements from the stack of open elements until an li element
            //     has been popped from the stack."
            Token::EndTag { name, .. } if name == "li" => {
                if self.has_element_in_list_item_scope("li") {
                    self.generate_implied_end_tags_excluding(Some("li"));
                    // If current node is not "li", this is a parse error (ignored).
                    self.pop_until_tag("li");
                }
                // else: parse error, ignore token
            }

            // [§ 13.2.6.4.7 "in body" - End tags "dd", "dt"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "An end tag whose tag name is one of: "dd", "dt""
            // "If the stack of open elements does not have an element in scope
            //  that is an HTML element with the same tag name as that of the
            //  token, then this is a parse error; ignore the token."
            // "Otherwise, run these steps:
            //  1. Generate implied end tags, except for HTML elements with the
            //     same tag name as the token.
            //  2. If the current node is not an HTML element with the same tag
            //     name as the token, then this is a parse error.
            //  3. Pop elements from the stack of open elements until an HTML
            //     element with the same tag name as the token has been popped."
            Token::EndTag { name, .. } if matches!(name.as_str(), "dd" | "dt") => {
                if self.has_element_in_scope(name) {
                    self.generate_implied_end_tags_excluding(Some(name));
                    // If current node is not the target, this is a parse error (ignored).
                    self.pop_until_tag(name);
                }
                // else: parse error, ignore token
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "table"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "table""
            // "If the Document is not set to quirks mode, and the stack of open elements has a p
            //  element in button scope, then close a p element."
            // "Insert an HTML element for the token."
            // "Set the frameset-ok flag to "not ok"."
            // "Switch the insertion mode to "in table"."
            Token::StartTag { name, .. } if name == "table" => {
                // TODO: Check quirks mode flag before closing p.
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InTable;
            }

            // "A start tag whose tag name is one of: "area", "br", "embed", "img", "keygen", "wbr""
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token. Immediately pop the current node off the
            // stack of open elements."
            // "Acknowledge the token's self-closing flag, if it is set."
            // "Set the frameset-ok flag to "not ok"."
            //
            // "A start tag whose tag name is "input""
            // (similar handling for void element)
            //
            // "A start tag whose tag name is "hr""
            // (similar handling for void element)
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "area" | "br" | "embed" | "img" | "keygen" | "wbr" | "input" | "hr"
                ) =>
            {
                let _ = self.insert_html_element(token);
                let _ = self.stack_of_open_elements.pop();
            }

            // [§ 13.2.6.4.7 "in body"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "base", "basefont", "bgsound", "link",
            // "meta", "noframes", "script", "style", "template", "title""
            // "Process the token using the rules for the "in head" insertion mode."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "base"
                        | "basefont"
                        | "bgsound"
                        | "link"
                        | "meta"
                        | "noframes"
                        | "script"
                        | "style"
                        | "template"
                        | "title"
                ) =>
            {
                self.handle_in_head_mode(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "noscript"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // The behavior depends on whether the scripting flag is enabled or disabled.
            // Since this browser has no JavaScript engine, scripting is effectively disabled.
            Token::StartTag { name, .. } if name == "noscript" => {
                // CASE A: If the scripting flag is ENABLED:
                // "Follow the generic raw text element parsing algorithm."
                //
                // [§ 13.2.6.3 Generic raw text element parsing algorithm](https://html.spec.whatwg.org/multipage/parsing.html#generic-raw-text-element-parsing-algorithm):
                //   1. Insert an HTML element for the token.
                //   2. Let the original insertion mode be the current insertion mode.
                //   3. Switch the insertion mode to "text".
                //
                // (This treats <noscript> contents as raw text, not parsed HTML)

                // CASE B: If the scripting flag is DISABLED (our case):
                // "Reconstruct the active formatting elements, if any."
                // "Insert an HTML element for the token."
                // "Switch the insertion mode to "in head noscript"."
                //
                // (This parses <noscript> contents as HTML since scripts won't run)

                // STEP 1: Reconstruct active formatting elements.
                // "Reconstruct the active formatting elements, if any."
                self.reconstruct_active_formatting_elements();

                // STEP 2: Insert the noscript element.
                // "Insert an HTML element for the token."
                let _ = self.insert_html_element(token);

                // STEP 3: Switch insertion mode.
                // "Switch the insertion mode to "in head noscript"."
                self.insertion_mode = InsertionMode::InHeadNoscript;
            }

            // "An end tag whose tag name is one of: "address", "article", "aside", "blockquote",
            // "button", "center", "details", "dialog", "dir", "div", "dl", "fieldset", "figcaption",
            // "figure", "footer", "header", "hgroup", "listing", "main", "menu", "nav", "ol", "pre",
            // "search", "section", "summary", "ul""
            // "If the stack of open elements does not have an element in scope that is an HTML
            // element with the same tag name as that of the token, then this is a parse error;
            // ignore the token."
            // "Otherwise, run these steps:"
            // 1. "Generate implied end tags."
            // 2. "If the current node is not an HTML element with the same tag name as that of
            //     the token, then this is a parse error."
            // 3. "Pop elements from the stack of open elements until an HTML element with the
            //     same tag name as the token has been popped from the stack."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "address"
                        | "article"
                        | "aside"
                        | "blockquote"
                        | "button"
                        | "center"
                        | "details"
                        | "dialog"
                        | "dir"
                        | "div"
                        | "dl"
                        | "fieldset"
                        | "figcaption"
                        | "figure"
                        | "footer"
                        | "header"
                        | "hgroup"
                        | "listing"
                        | "main"
                        | "menu"
                        | "nav"
                        | "ol"
                        | "pre"
                        | "search"
                        | "section"
                        | "summary"
                        | "ul"
                ) =>
            {
                // STEP 1: Check if element is in scope.
                if !self.has_element_in_scope(name) {
                    // "this is a parse error; ignore the token."
                    return;
                }
                // STEP 2: Generate implied end tags.
                self.generate_implied_end_tags();
                // STEP 3: If current node is not the target, this is a parse error (ignored).
                // STEP 4: Pop until target is popped.
                self.pop_until_tag(name);
            }

            // [§ 13.2.6.4.7 "in body" - End tag h1-h6](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "An end tag whose tag name is one of: "h1", "h2", "h3", "h4", "h5", "h6""
            // "If the stack of open elements does not have an element in scope that is an HTML
            // element and whose tag name is one of "h1", "h2", "h3", "h4", "h5", "h6", then this
            // is a parse error; ignore the token."
            // "Otherwise, run these steps:"
            // 1. "Generate implied end tags."
            // 2. "If the current node is not an HTML element with the same tag name as that of
            //     the token, then this is a parse error."
            // 3. "Pop elements from the stack of open elements until an HTML element whose tag
            //     name is one of "h1", "h2", "h3", "h4", "h5", "h6" has been popped from the stack."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") =>
            {
                // Pop until any heading element is found (spec allows closing h2 with </h1>, etc.)
                self.pop_until_one_of(&["h1", "h2", "h3", "h4", "h5", "h6"]);
            }

            // [§ 13.2.6.4.7 "in body" - End tag "p"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "An end tag whose tag name is "p""
            // "If the stack of open elements does not have a p element in button scope, then
            // this is a parse error; act as if a start tag with the tag name "p" had been seen,
            // then reprocess the current token."
            // "Otherwise, run these steps:"
            // 1. "Generate implied end tags, except for p elements."
            // 2. "If the current node is not a p element, then this is a parse error."
            // 3. "Pop elements from the stack of open elements until a p element has been
            //     popped from the stack."
            Token::EndTag { name, .. } if name == "p" => {
                if self.has_element_in_button_scope("p") {
                    // STEP 1: "Generate implied end tags, except for p elements."
                    self.generate_implied_end_tags_excluding(Some("p"));
                    // STEP 3: "Pop elements from the stack until a p element has been popped."
                    self.pop_until_tag("p");
                } else {
                    // "If the stack of open elements does not have a p element in button scope,
                    //  then this is a parse error; act as if a start tag with the tag name "p"
                    //  had been seen, then reprocess the current token."
                    let fake_p = Token::StartTag {
                        name: "p".to_string(),
                        self_closing: false,
                        attributes: Vec::new(),
                    };
                    let _ = self.insert_html_element(&fake_p);
                    self.reprocess_token(token);
                }
            }

            // [§ 13.2.6.4.7 "in body" - End tag "applet", "marquee", "object"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "An end tag whose tag name is one of: "applet", "marquee", "object""
            // "If the stack of open elements does not have an element in scope that is an HTML
            //  element with the same tag name as that of the token, then this is a parse error;
            //  ignore the token."
            // "Otherwise, run these steps:"
            // 1. "Generate implied end tags."
            // 2. "If the current node is not an HTML element with the same tag name as that of
            //     the token, then this is a parse error."
            // 3. "Pop elements from the stack of open elements until an HTML element with the
            //     same tag name as the token has been popped from the stack."
            // 4. "Clear the list of active formatting elements up to the last marker."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "applet" | "marquee" | "object") =>
            {
                if self.has_element_in_scope(name) {
                    self.generate_implied_end_tags();
                    self.pop_until_tag(name);
                    // "Clear the list of active formatting elements up to the last marker."
                    self.clear_active_formatting_elements_to_last_marker();
                }
                // Otherwise: parse error, ignore the token
            }

            // [§ 13.2.6.4.7 "in body" - End tag "template"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "An end tag whose tag name is "template""
            // "Process the token using the rules for the "in head" insertion mode."
            Token::EndTag { name, .. } if name == "template" => {
                self.handle_in_head_mode(token);
            }

            // [§ 13.2.6.4.7 "in body" - End tag "select"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "An end tag whose tag name is "select""
            // "Parse error."
            // "If the stack of open elements does not have a select element in select scope,
            //  ignore the token. (fragment case)"
            // "Otherwise:"
            // "Pop elements from the stack of open elements until a select element has been
            //  popped from the stack."
            // "Reset the insertion mode appropriately."
            Token::EndTag { name, .. } if name == "select" => {
                // NOTE: Using has_element_in_scope instead of select scope (simplified)
                if self.has_element_in_scope("select") {
                    self.pop_until_tag("select");
                    // TODO: Reset the insertion mode appropriately
                }
                // Otherwise: ignore the token (fragment case or parse error)
            }

            // [§ 13.2.6.4.7 "in body" - End tags "optgroup", "option"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // These fall under "Any other end tag" rules since there's no specific
            // handler in InBody mode. Using simplified pop-until-tag behavior.
            Token::EndTag { name, .. } if matches!(name.as_str(), "optgroup" | "option") => {
                if self.has_element_in_scope(name) {
                    self.pop_until_tag(name);
                }
                // Otherwise: ignore the token
            }

            // [§ 13.2.6.4.7 "in body" - End tag "iframe", "noembed", "noframes"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // These are raw text elements, end tags follow "any other end tag" rules.
            // NOTE: Simplified implementation.
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "iframe" | "noembed" | "noframes" | "noscript"
                ) =>
            {
                if self.has_element_in_scope(name) {
                    // TODO: generate_implied_end_tags(Some(name)) before popping
                    self.pop_until_tag(name);
                }
                // Otherwise: parse error, ignore the token (stray end tag)
            }

            // [§ 13.2.6.4.7 "in body" - End tags "svg", "math"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // These are foreign content elements. End tags fall under "Any other end tag" rules:
            // [§ 13.2.6.4.7 Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // 1. "Initialize node to be the current node (the bottommost node of the stack)."
            // 2. "Loop: If node is an HTML element with the same tag name as the token, then:"
            //    a. "Generate implied end tags, except for HTML elements with the same tag name."
            //    b. "If node is not the current node, then this is a parse error."
            //    c. "Pop all the nodes from the current node up to node, including node, then stop."
            // 3. "Otherwise, if node is in the special category, parse error; ignore the token."
            // 4. "Set node to the previous entry in the stack of open elements."
            // 5. "Return to the step labeled loop."
            Token::EndTag { name, .. } if matches!(name.as_str(), "svg" | "math") => {
                if self.has_element_in_scope(name) {
                    self.pop_until_tag(name);
                }
                // Otherwise: parse error, ignore the token
            }

            // [§ 13.2.6.4.7 "in body" - Formatting element end tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "An end tag whose tag name is one of: "a", "b", "big", "code", "em", "font",
            //  "i", "nobr", "s", "small", "strike", "strong", "tt", "u""
            //
            // "Run the adoption agency algorithm for the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "a" | "b"
                        | "big"
                        | "code"
                        | "em"
                        | "font"
                        | "i"
                        | "nobr"
                        | "s"
                        | "small"
                        | "strike"
                        | "strong"
                        | "tt"
                        | "u"
                ) =>
            {
                self.run_adoption_agency(name);
            }

            // [§ 13.2.6.4.7 "in body" - Other inline end tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // Non-formatting inline end tags use "any other end tag" algorithm.
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "span"
                        | "label"
                        | "cite"
                        | "q"
                        | "dfn"
                        | "abbr"
                        | "ruby"
                        | "rt"
                        | "rp"
                        | "data"
                        | "time"
                        | "var"
                        | "samp"
                        | "kbd"
                        | "sub"
                        | "sup"
                        | "mark"
                        | "bdi"
                        | "bdo"
                        | "wbr"
                ) =>
            {
                self.any_other_end_tag(name);
            }

            // [§ 13.2.6.4.7 "in body" - End tag "form"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "An end tag whose tag name is "form""
            // (Complex handling involving form element pointer - simplified here)
            Token::EndTag { name, .. } if name == "form" => {
                // NOTE: Simplified - just pop until form
                self.pop_until_tag("form");
            }

            // "An end tag whose tag name is "body""
            // "If the stack of open elements does not have a body element in scope, this is a
            // parse error; ignore the token."
            // "Otherwise, if there is a node in the stack of open elements that is not either a
            // dd element, a dt element, an li element, an optgroup element, an option element,
            // a p element, an rb element, an rp element, an rt element, an rtc element, a tbody
            // element, a td element, a tfoot element, a th element, a thead element, a tr element,
            // the body element, or the html element, then this is a parse error."
            // "Switch the insertion mode to "after body"."
            Token::EndTag { name, .. } if name == "body" => {
                self.insertion_mode = InsertionMode::AfterBody;
            }

            // "An end tag whose tag name is "html""
            // "If the stack of open elements does not have a body element in scope, this is a
            // parse error; ignore the token."
            // "Otherwise, if there is a node in the stack of open elements that is not either
            // [list of elements], then this is a parse error."
            // "Switch the insertion mode to "after body"."
            // "Reprocess the token."
            Token::EndTag { name, .. } if name == "html" => {
                self.insertion_mode = InsertionMode::AfterBody;
                self.reprocess_token(token);
            }

            // "An end-of-file token"
            // "If the stack of template insertion modes is not empty, then process the token
            // using the rules for the "in template" insertion mode."
            // "Otherwise, follow these steps:"
            // 1. "If there is a node in the stack of open elements that is not either [list],
            //     then this is a parse error."
            // 2. "Stop parsing."
            Token::EndOfFile => {
                self.stopped = true;
            }

            // [§ 13.2.6.4.7 "in body"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is "svg""
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } if name == "svg" => {
                // "Reconstruct the active formatting elements, if any."
                self.reconstruct_active_formatting_elements();

                // "Adjust SVG attributes for the token. (This fixes the case of
                // SVG attributes that are not all lowercase.)"
                //
                // "Adjust foreign attributes for the token. (This fixes the use
                // of namespaced attributes, in particular XLink in SVG.)"
                let mut adjusted_attributes = attributes.clone();
                adjust_svg_attributes(&mut adjusted_attributes);
                adjust_foreign_attributes(&mut adjusted_attributes);

                // "Insert a foreign element for the token, with SVG namespace
                // and false."
                let adjusted_token = Token::StartTag {
                    name: name.clone(),
                    attributes: adjusted_attributes,
                    self_closing: *self_closing,
                };
                let _ = self.insert_foreign_element(&adjusted_token, Namespace::Svg);

                // "If the token has its self-closing flag set, pop the current
                // node off the stack of open elements and acknowledge the
                // token's self-closing flag."
                if *self_closing {
                    let _ = self.stack_of_open_elements.pop();
                }
            }

            // "A start tag whose tag name is "math""
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } if name == "math" => {
                // "Reconstruct the active formatting elements, if any."
                self.reconstruct_active_formatting_elements();

                // "Adjust MathML attributes for the token. (This fixes the case
                // of MathML attributes that are not all lowercase.)"
                //
                // "Adjust foreign attributes for the token. (This fixes the use
                // of namespaced attributes, in particular XLink.)"
                let mut adjusted_attributes = attributes.clone();
                adjust_mathml_attributes(&mut adjusted_attributes);
                adjust_foreign_attributes(&mut adjusted_attributes);

                // "Insert a foreign element for the token, with MathML
                // namespace and false."
                let adjusted_token = Token::StartTag {
                    name: name.clone(),
                    attributes: adjusted_attributes,
                    self_closing: *self_closing,
                };
                let _ = self.insert_foreign_element(&adjusted_token, Namespace::MathMl);

                // "If the token has its self-closing flag set, pop the current
                // node off the stack of open elements and acknowledge the
                // token's self-closing flag."
                if *self_closing {
                    let _ = self.stack_of_open_elements.pop();
                }
            }

            // [§ 13.2.6.4.7 "in body" - Any other start tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "Any other start tag"
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            //
            // This handles all elements not explicitly listed in the spec, including:
            // - Custom elements (contain hyphen, e.g., <my-widget>)
            // - Web component elements (slot)
            // - Text-level semantics (ins, del, abbr, dfn, time, data, code, var, samp, kbd,
            //   mark, ruby, rt, rp, bdi, bdo, q, cite, sub, sup, small, etc.)
            // - Any other valid HTML element without special parsing rules
            Token::StartTag { .. } => {
                self.reconstruct_active_formatting_elements();
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "Any other end tag"
            Token::EndTag { name, .. } => {
                self.any_other_end_tag(name);
            }
        }
    }

    /// [§ 13.2.6.4.9 The "in table" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
    ///
    /// "When the user agent is to apply the rules for the "in table" insertion
    /// mode, the user agent must handle the token as follows:"
    fn handle_in_table_mode(&mut self, token: &Token) {
        match token {
            // "A character token, if the current node is table, tbody, tfoot, thead, or tr
            //  element:"
            // "Let the pending table character tokens be an empty list of tokens."
            // "Let the original insertion mode be the current insertion mode."
            // "Switch the insertion mode to "in table text" and reprocess the token."
            Token::Character { .. } => {
                if let Some(current) = self.current_node()
                    && let Some(tag) = self.get_tag_name(current)
                    && matches!(tag, "table" | "tbody" | "tfoot" | "thead" | "tr")
                {
                    self.pending_table_character_tokens.clear();
                    self.original_insertion_mode = Some(self.insertion_mode);
                    self.insertion_mode = InsertionMode::InTableText;
                    self.reprocess_token(token);
                } else {
                    // "Anything else" — foster parent via InBody
                    self.handle_in_table_anything_else(token);
                }
            }

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

            // "A start tag whose tag name is "caption""
            // "Clear the stack back to a table context."
            // "Insert a marker at the end of the list of active formatting elements."
            // "Insert an HTML element for the token, then switch the insertion mode
            //  to "in caption"."
            Token::StartTag { name, .. } if name == "caption" => {
                self.clear_stack_back_to_table_context();
                self.active_formatting_elements
                    .push(ActiveFormattingElement::Marker);
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InCaption;
            }

            // "A start tag whose tag name is "colgroup""
            // "Clear the stack back to a table context."
            // "Insert an HTML element for the token, then switch the insertion mode
            //  to "in column group"."
            Token::StartTag { name, .. } if name == "colgroup" => {
                self.clear_stack_back_to_table_context();
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InColumnGroup;
            }

            // "A start tag whose tag name is "col""
            // "Clear the stack back to a table context."
            // "Insert an HTML element for a "colgroup" start tag token with no attributes,
            //  then switch the insertion mode to "in column group"."
            // "Reprocess the current token."
            Token::StartTag { name, .. } if name == "col" => {
                self.clear_stack_back_to_table_context();
                let fake_colgroup = Token::StartTag {
                    name: "colgroup".to_string(),
                    self_closing: false,
                    attributes: Vec::new(),
                };
                let _ = self.insert_html_element(&fake_colgroup);
                self.insertion_mode = InsertionMode::InColumnGroup;
                self.reprocess_token(token);
            }

            // "A start tag whose tag name is one of: "tbody", "tfoot", "thead""
            // "Clear the stack back to a table context."
            // "Insert an HTML element for the token, then switch the insertion mode
            //  to "in table body"."
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "tbody" | "tfoot" | "thead") =>
            {
                self.clear_stack_back_to_table_context();
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InTableBody;
            }

            // "A start tag whose tag name is one of: "td", "th", "tr""
            // "Clear the stack back to a table context."
            // "Insert an HTML element for a "tbody" start tag token with no attributes,
            //  then switch the insertion mode to "in table body"."
            // "Reprocess the current token."
            Token::StartTag { name, .. } if matches!(name.as_str(), "td" | "th" | "tr") => {
                self.clear_stack_back_to_table_context();
                let fake_tbody = Token::StartTag {
                    name: "tbody".to_string(),
                    self_closing: false,
                    attributes: Vec::new(),
                };
                let _ = self.insert_html_element(&fake_tbody);
                self.insertion_mode = InsertionMode::InTableBody;
                self.reprocess_token(token);
            }

            // "A start tag whose tag name is "table""
            // "Parse error."
            // "If the stack of open elements does not have a table element in
            //  table scope, ignore the token."
            // "Otherwise:"
            //   "Pop elements from the stack of open elements until a table element
            //    has been popped from the stack."
            //   "Reset the insertion mode appropriately."
            //   "Reprocess the token."
            Token::StartTag { name, .. } if name == "table" => {
                if self.has_element_in_table_scope("table") {
                    self.pop_until_tag("table");
                    self.reset_insertion_mode_appropriately();
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore (no table in scope)
            }

            // "An end tag whose tag name is "table""
            // "If the stack of open elements does not have a table element in
            //  table scope, this is a parse error; ignore the token."
            // "Otherwise:"
            //   "Pop elements from the stack of open elements until a table element
            //    has been popped from the stack."
            //   "Reset the insertion mode appropriately."
            Token::EndTag { name, .. } if name == "table" => {
                if self.has_element_in_table_scope("table") {
                    self.pop_until_tag("table");
                    self.reset_insertion_mode_appropriately();
                } else {
                    // Parse error. Ignore the token.
                }
            }

            // "An end tag whose tag name is one of: "body", "caption", "col",
            //  "colgroup", "html", "tbody", "td", "tfoot", "th", "thead", "tr""
            // "Parse error. Ignore the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "body"
                        | "caption"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                // Parse error. Ignore the token.
            }

            // "A start tag whose tag name is one of: "style", "script", "template""
            // "An end tag whose tag name is "template""
            // "Process the token using the rules for the "in head" insertion mode."
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "style" | "script" | "template") =>
            {
                self.handle_in_head_mode(token);
            }
            Token::EndTag { name, .. } if name == "template" => {
                self.handle_in_head_mode(token);
            }

            // "A start tag whose tag name is "input""
            // "If the token does not have an attribute with the name "type",
            //  or if it does, but that attribute's value is not an ASCII
            //  case-insensitive match for the string "hidden", then: act as
            //  described in the "anything else" entry below."
            // "Otherwise:"
            //   "Parse error."
            //   "Insert an HTML element for the token."
            //   "Pop that input element off the stack of open elements."
            //   "Acknowledge the token's self-closing flag, if it is set."
            Token::StartTag {
                name, attributes, ..
            } if name == "input" => {
                let is_hidden = attributes.iter().any(|attr| {
                    attr.name.eq_ignore_ascii_case("type")
                        && attr.value.eq_ignore_ascii_case("hidden")
                });
                if is_hidden {
                    // Parse error. Insert element and pop immediately.
                    let _ = self.insert_html_element(token);
                    let _ = self.stack_of_open_elements.pop();
                } else {
                    // Not hidden — treat as "anything else" (foster parent)
                    self.handle_in_table_anything_else(token);
                }
            }

            // "A start tag whose tag name is "form""
            // "Parse error."
            // "If there is a template element on the stack of open elements,
            //  or if the form element pointer is not null, ignore the token."
            // "Otherwise:"
            //   "Insert an HTML element for the token, and set the form element
            //    pointer to point to the element created."
            //   "Pop that form element off the stack of open elements."
            Token::StartTag { name, .. } if name == "form" => {
                let has_template = self
                    .stack_of_open_elements
                    .iter()
                    .any(|&id| self.get_tag_name(id) == Some("template"));
                if has_template || self.form_element_pointer.is_some() {
                    // Parse error. Ignore the token.
                } else {
                    let form_id = self.insert_html_element(token);
                    self.form_element_pointer = Some(form_id);
                    let _ = self.stack_of_open_elements.pop();
                }
            }

            // "An end-of-file token"
            // "Process the token using the rules for the "in body" insertion mode."
            Token::EndOfFile => {
                self.handle_in_body_mode(token);
            }

            // "Anything else"
            _ => {
                self.handle_in_table_anything_else(token);
            }
        }
    }

    /// [§ 13.2.6.4.9 The "in table" insertion mode - Anything else](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
    ///
    /// "Anything else":
    /// "Parse error. Enable foster parenting, process the token using the rules
    /// for the "in body" insertion mode, and then disable foster parenting."
    fn handle_in_table_anything_else(&mut self, token: &Token) {
        // "Parse error."
        // "Enable foster parenting."
        self.foster_parenting = true;
        // "Process the token using the rules for the "in body" insertion mode."
        self.handle_in_body_mode(token);
        // "Disable foster parenting."
        self.foster_parenting = false;
    }
    /// [§ 13.2.6.4.10 The "in table text" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext)
    fn handle_in_table_text_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is U+0000 NULL"
            // "Parse error. Ignore the token."
            Token::Character { data: '\0' } => {}

            // "Any other character token"
            // "Append the character token to the pending table character tokens list."
            Token::Character { data } => {
                self.pending_table_character_tokens
                    .push(Token::Character { data: *data });
            }

            // "Anything else"
            _ => {
                if !self.pending_table_character_tokens.is_empty() {
                    // Drain the pending list before calling &mut self methods.
                    let pending = std::mem::take(&mut self.pending_table_character_tokens);

                    let all_whitespace = pending.iter().all(
                        |t| matches!(t, Token::Character { data } if Self::is_whitespace(*data)),
                    );

                    if all_whitespace {
                        // "If the pending table character tokens list consists
                        //  entirely of space characters, insert the characters."
                        for tok in &pending {
                            if let Token::Character { data } = tok {
                                self.insert_character(*data);
                            }
                        }
                    } else {
                        // "Otherwise, this is a parse error. Enable foster
                        //  parenting, process the pending table character tokens
                        //  using the rules for the "in body" insertion mode, and
                        //  disable foster parenting."
                        self.foster_parenting = true;
                        for tok in &pending {
                            self.handle_in_body_mode(tok);
                        }
                        self.foster_parenting = false;
                    }
                }

                // "Switch the insertion mode to the original insertion mode and
                //  reprocess the token."
                if let Some(original_mode) = self.original_insertion_mode.take() {
                    self.insertion_mode = original_mode;
                }
                self.reprocess_token(token);
            }
        }
    }

    /// [§ 13.2.6.4.13 The "in table body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intablebody)
    fn handle_in_table_body_mode(&mut self, token: &Token) {
        match token {
            // "A start tag whose tag name is "tr""
            // "Clear the stack back to a table body context."
            // "Insert an HTML element for the token, then switch the insertion
            //  mode to "in row"."
            Token::StartTag { name, .. } if name == "tr" => {
                self.clear_stack_back_to_table_body_context();
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InRow;
            }

            // "A start tag whose tag name is one of: "th", "td""
            // "Parse error."
            // "Clear the stack back to a table body context."
            // "Insert an HTML element for a "tr" start tag token with no
            //  attributes, then switch the insertion mode to "in row"."
            // "Reprocess the current token."
            Token::StartTag { name, .. } if matches!(name.as_str(), "th" | "td") => {
                self.clear_stack_back_to_table_body_context();
                let fake_tr = Token::StartTag {
                    name: "tr".to_string(),
                    self_closing: false,
                    attributes: Vec::new(),
                };
                let _ = self.insert_html_element(&fake_tr);
                self.insertion_mode = InsertionMode::InRow;
                self.reprocess_token(token);
            }

            // "An end tag whose tag name is one of: "tbody", "tfoot", "thead""
            // "If the stack of open elements does not have an element in table
            //  scope that is an HTML element with the same tag name as the
            //  token, this is a parse error; ignore the token."
            // "Otherwise:"
            //   "Clear the stack back to a table body context."
            //   "Pop the current node from the stack of open elements. Switch
            //    the insertion mode to "in table"."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "tbody" | "tfoot" | "thead") =>
            {
                if self.has_element_in_table_scope(name) {
                    self.clear_stack_back_to_table_body_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTable;
                }
                // Otherwise: parse error, ignore.
            }

            // "A start tag whose tag name is one of: "caption", "col",
            //  "colgroup", "tbody", "tfoot", "thead""
            // "An end tag whose tag name is "table""
            // "If the stack of open elements does not have a tbody, thead, or
            //  tfoot element in table scope, this is a parse error; ignore
            //  the token."
            // "Otherwise:"
            //   "Clear the stack back to a table body context."
            //   "Pop the current node from the stack of open elements. Switch
            //    the insertion mode to "in table"."
            //   "Reprocess the token."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead"
                ) =>
            {
                if self.has_element_in_table_scope("tbody")
                    || self.has_element_in_table_scope("thead")
                    || self.has_element_in_table_scope("tfoot")
                {
                    self.clear_stack_back_to_table_body_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTable;
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }
            Token::EndTag { name, .. } if name == "table" => {
                if self.has_element_in_table_scope("tbody")
                    || self.has_element_in_table_scope("thead")
                    || self.has_element_in_table_scope("tfoot")
                {
                    self.clear_stack_back_to_table_body_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTable;
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }

            // "An end tag whose tag name is one of: "body", "caption", "col",
            //  "colgroup", "html", "td", "th", "tr""
            // "Parse error. Ignore the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" | "tr"
                ) => {}

            // "Anything else"
            // "Process the token using the rules for the "in table" insertion mode."
            _ => {
                self.handle_in_table_mode(token);
            }
        }
    }

    /// [§ 13.2.6.4.14 The "in row" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow)
    fn handle_in_row_mode(&mut self, token: &Token) {
        match token {
            // "A start tag whose tag name is one of: "th", "td""
            // "Clear the stack back to a table row context."
            // "Insert an HTML element for the token, then switch the insertion
            //  mode to "in cell"."
            // "Insert a marker at the end of the list of active formatting elements."
            Token::StartTag { name, .. } if matches!(name.as_str(), "th" | "td") => {
                self.clear_stack_back_to_table_row_context();
                let _ = self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InCell;
                self.active_formatting_elements
                    .push(ActiveFormattingElement::Marker);
            }

            // "An end tag whose tag name is "tr""
            // "If the stack of open elements does not have a tr element in
            //  table scope, this is a parse error; ignore the token."
            // "Otherwise:"
            //   "Clear the stack back to a table row context."
            //   "Pop the current node (which will be a tr element) from the
            //    stack of open elements. Switch the insertion mode to
            //    "in table body"."
            Token::EndTag { name, .. } if name == "tr" => {
                if self.has_element_in_table_scope("tr") {
                    self.clear_stack_back_to_table_row_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTableBody;
                }
                // Otherwise: parse error, ignore.
            }

            // "A start tag whose tag name is one of: "caption", "col",
            //  "colgroup", "tbody", "tfoot", "thead", "tr""
            // "An end tag whose tag name is "table""
            // "If the stack of open elements does not have a tr element in
            //  table scope, this is a parse error; ignore the token."
            // "Otherwise:"
            //   "Clear the stack back to a table row context."
            //   "Pop the current node (which will be a tr element) from the
            //    stack of open elements. Switch the insertion mode to
            //    "in table body"."
            //   "Reprocess the token."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr"
                ) =>
            {
                if self.has_element_in_table_scope("tr") {
                    self.clear_stack_back_to_table_row_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTableBody;
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }
            Token::EndTag { name, .. } if name == "table" => {
                if self.has_element_in_table_scope("tr") {
                    self.clear_stack_back_to_table_row_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTableBody;
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }

            // "An end tag whose tag name is one of: "tbody", "tfoot", "thead""
            // "If the stack of open elements does not have an element in table
            //  scope that is an HTML element with the same tag name as the
            //  token, this is a parse error; ignore the token."
            // "If the stack of open elements does not have a tr element in
            //  table scope, ignore the token."
            // "Otherwise:"
            //   "Clear the stack back to a table row context."
            //   "Pop the current node (which will be a tr element) from the
            //    stack of open elements. Switch the insertion mode to
            //    "in table body"."
            //   "Reprocess the token."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "tbody" | "tfoot" | "thead") =>
            {
                if !self.has_element_in_table_scope(name) {
                    // Parse error, ignore.
                } else if !self.has_element_in_table_scope("tr") {
                    // Ignore.
                } else {
                    self.clear_stack_back_to_table_row_context();
                    let _ = self.stack_of_open_elements.pop();
                    self.insertion_mode = InsertionMode::InTableBody;
                    self.reprocess_token(token);
                }
            }

            // "An end tag whose tag name is one of: "body", "caption", "col",
            //  "colgroup", "html", "td", "th""
            // "Parse error. Ignore the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th"
                ) => {}

            // "Anything else"
            // "Process the token using the rules for the "in table" insertion mode."
            _ => {
                self.handle_in_table_mode(token);
            }
        }
    }

    /// [§ 13.2.6.4.15 The "in cell" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incell)
    fn handle_in_cell_mode(&mut self, token: &Token) {
        match token {
            // "An end tag whose tag name is one of: "td", "th""
            // "If the stack of open elements does not have an element in table
            //  scope that is an HTML element with the same tag name as that of
            //  the token, then this is a parse error; ignore the token."
            // "Otherwise:"
            //   "Generate implied end tags."
            //   "If the current node is not an HTML element with the same tag
            //    name as the token, then this is a parse error."
            //   "Pop elements from the stack of open elements stack until an
            //    HTML element with the same tag name as the token has been
            //    popped from the stack."
            //   "Clear the list of active formatting elements up to the last
            //    marker."
            //   "Switch the insertion mode to "in row"."
            Token::EndTag { name, .. } if matches!(name.as_str(), "td" | "th") => {
                if self.has_element_in_table_scope(name) {
                    self.generate_implied_end_tags();
                    self.pop_until_tag(name);
                    self.clear_active_formatting_elements_to_last_marker();
                    self.insertion_mode = InsertionMode::InRow;
                }
                // Otherwise: parse error, ignore.
            }

            // "A start tag whose tag name is one of: "caption", "col",
            //  "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr""
            // "If the stack of open elements does not have a td or th element
            //  in table scope, then this is a parse error; ignore the token."
            // "Otherwise, close the cell and reprocess the token."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                if self.has_element_in_table_scope("td")
                    || self.has_element_in_table_scope("th")
                {
                    self.close_the_cell();
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }

            // "An end tag whose tag name is one of: "body", "caption", "col",
            //  "colgroup", "html""
            // "Parse error. Ignore the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html"
                ) => {}

            // "An end tag whose tag name is one of: "table", "tbody", "tfoot",
            //  "thead", "tr""
            // "If the stack of open elements does not have an element in table
            //  scope that is an HTML element with the same tag name as that of
            //  the token, then this is a parse error; ignore the token."
            // "Otherwise, close the cell and reprocess the token."
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "table" | "tbody" | "tfoot" | "thead" | "tr"
                ) =>
            {
                if self.has_element_in_table_scope(name) {
                    self.close_the_cell();
                    self.reprocess_token(token);
                }
                // Otherwise: parse error, ignore.
            }

            // "Anything else"
            // "Process the token using the rules for the "in body" insertion mode."
            _ => {
                self.handle_in_body_mode(token);
            }
        }
    }

    /// [§ 13.2.6.4.19 The "after body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterbody)
    fn handle_after_body_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "Process the token using the rules for the "in body" insertion mode."
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.handle_in_body_mode(token);
            }

            // "A comment token"
            // "Insert a comment as the last child of the first element in the stack of open
            // elements (the html element)."
            Token::Comment { data } => {
                if let Some(&html_idx) = self.stack_of_open_elements.first() {
                    let comment_idx = self.create_comment_node(data.clone());
                    self.append_child(html_idx, comment_idx);
                }
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

            // "A start tag whose tag name is "html""
            // "Process the token using the rules for the "in body" insertion mode."
            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // "An end tag whose tag name is "html""
            // "If the parser was created as part of the HTML fragment parsing algorithm, this is
            // a parse error; ignore the token. (fragment case)"
            // "Otherwise, switch the insertion mode to "after after body"."
            Token::EndTag { name, .. } if name == "html" => {
                self.insertion_mode = InsertionMode::AfterAfterBody;
            }

            // "An end-of-file token"
            // "Stop parsing."
            Token::EndOfFile => {
                self.stopped = true;
            }

            // "Anything else"
            // "Parse error. Switch the insertion mode to "in body" and reprocess the token."
            _ => {
                self.insertion_mode = InsertionMode::InBody;
                self.reprocess_token(token);
            }
        }
    }

    /// [§ 13.2.6.4.22 The "after after body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-body-insertion-mode)
    fn handle_after_after_body_mode(&mut self, token: &Token) {
        match token {
            // "A comment token"
            // "Insert a comment as the last child of the Document object."
            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            // "A DOCTYPE token"
            // "A character token that is one of U+0009 CHARACTER TABULATION, U+000A LINE FEED (LF),
            // U+000C FORM FEED (FF), U+000D CARRIAGE RETURN (CR), or U+0020 SPACE"
            // "A start tag whose tag name is "html""
            // "Process the token using the rules for the "in body" insertion mode."
            Token::Doctype { .. } => {
                self.handle_in_body_mode(token);
            }

            Token::Character { data } if Self::is_whitespace(*data) => {
                self.handle_in_body_mode(token);
            }

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // "An end-of-file token"
            // "Stop parsing."
            Token::EndOfFile => {
                self.stopped = true;
            }

            // "Anything else"
            // "Parse error. Switch the insertion mode to "in body" and reprocess the token."
            _ => {
                self.insertion_mode = InsertionMode::InBody;
                self.reprocess_token(token);
            }
        }
    }
}

/// Print a DOM tree for debugging.
pub fn print_tree(tree: &DomTree, id: NodeId, indent: usize) {
    let prefix = "  ".repeat(indent);
    if let Some(node) = tree.get(id) {
        match &node.node_type {
            NodeType::Document => {
                println!("{prefix}Document");
            }
            NodeType::Element(data) => {
                if data.attrs.is_empty() {
                    println!("{prefix}<{}>", data.tag_name);
                } else {
                    let attrs: Vec<String> = data
                        .attrs
                        .iter()
                        .map(|(k, v)| {
                            if v.is_empty() {
                                k.clone()
                            } else {
                                format!("{k}=\"{v}\"")
                            }
                        })
                        .collect();
                    println!("{prefix}<{} {}>", data.tag_name, attrs.join(" "));
                }
            }
            NodeType::Text(data) => {
                let display = data.replace('\n', "\\n").replace(' ', "\u{00B7}");
                println!("{prefix}\"{display}\"");
            }
            NodeType::Comment(data) => {
                println!("{prefix}<!-- {data} -->");
            }
        }
        for &child_id in tree.children(id) {
            print_tree(tree, child_id, indent + 1);
        }
    }
}
