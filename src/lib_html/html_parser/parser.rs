use strum_macros::Display;

use crate::lib_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};
use crate::lib_html::html_tokenizer::token::{Attribute, Token};

/// [§ 13.2.4.1 The insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#the-insertion-mode)
///
/// "The insertion mode is a state variable that controls the primary operation
/// of the tree construction stage."
#[derive(Debug, Clone, Copy, PartialEq, Display)]
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

/// A parse error or warning encountered during parsing.
#[derive(Debug, Clone)]
pub struct ParseIssue {
    pub message: String,
    pub token_index: usize,
    pub is_error: bool,
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
    /// Stores NodeIds into the arena.
    stack_of_open_elements: Vec<NodeId>,

    /// [§ 13.2.4.4 The element pointers](https://html.spec.whatwg.org/multipage/parsing.html#the-element-pointers)
    head_element_pointer: Option<NodeId>,

    /// DOM tree with parent/sibling pointers.
    /// NodeId::ROOT (index 0) is the Document node.
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
}

impl HTMLParser {
    /// Create a new parser from a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        // DomTree::new() creates the Document node at NodeId::ROOT
        HTMLParser {
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: None,
            stack_of_open_elements: Vec::new(),
            head_element_pointer: None,
            tree: DomTree::new(),
            tokens,
            token_index: 0,
            stopped: false,
            issues: Vec::new(),
            strict_mode: false,
        }
    }

    /// Enable strict mode - panics on unhandled tokens.
    pub fn with_strict_mode(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Get all parse issues (errors and warnings) encountered during parsing.
    pub fn get_issues(&self) -> &[ParseIssue] {
        &self.issues
    }

    /// Record a parse warning (for unhandled but recoverable situations).
    fn parse_warning(&mut self, message: &str) {
        self.issues.push(ParseIssue {
            message: message.to_string(),
            token_index: self.token_index,
            is_error: false,
        });
    }

    /// Run the parser and return the DOM tree.
    ///
    /// The returned DomTree preserves parent/sibling relationships
    /// for efficient traversal.
    pub fn run(mut self) -> DomTree {
        while !self.stopped && self.token_index < self.tokens.len() {
            let token = self.tokens[self.token_index].clone();
            self.process_token(&token);
            self.token_index += 1;
        }
        self.tree
    }

    /// Run the parser and return both the DomTree and any parse issues.
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
    fn process_token(&mut self, token: &Token) {
        match self.insertion_mode {
            InsertionMode::Initial => self.handle_initial_mode(token),
            InsertionMode::BeforeHtml => self.handle_before_html_mode(token),
            InsertionMode::BeforeHead => self.handle_before_head_mode(token),
            InsertionMode::InHead => self.handle_in_head_mode(token),
            InsertionMode::InHeadNoscript => todo!("InHeadNoscript mode"),
            InsertionMode::AfterHead => self.handle_after_head_mode(token),
            InsertionMode::InBody => self.handle_in_body_mode(token),
            InsertionMode::Text => self.handle_text_mode(token),
            InsertionMode::InTable => todo!("InTable mode"),
            InsertionMode::InTableText => todo!("InTableText mode"),
            InsertionMode::InCaption => todo!("InCaption mode"),
            InsertionMode::InColumnGroup => todo!("InColumnGroup mode"),
            InsertionMode::InTableBody => todo!("InTableBody mode"),
            InsertionMode::InRow => todo!("InRow mode"),
            InsertionMode::InCell => todo!("InCell mode"),
            InsertionMode::InSelect => todo!("InSelect mode"),
            InsertionMode::InSelectInTable => todo!("InSelectInTable mode"),
            InsertionMode::InTemplate => todo!("InTemplate mode"),
            InsertionMode::AfterBody => self.handle_after_body_mode(token),
            InsertionMode::InFrameset => todo!("InFrameset mode"),
            InsertionMode::AfterFrameset => todo!("AfterFrameset mode"),
            InsertionMode::AfterAfterBody => self.handle_after_after_body_mode(token),
            InsertionMode::AfterAfterFrameset => todo!("AfterAfterFrameset mode"),
        }
    }

    fn reprocess_token(&mut self, token: &Token) {
        self.process_token(token);
    }

    fn is_whitespace(c: char) -> bool {
        matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ')
    }

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#current-node)
    fn current_node(&self) -> Option<NodeId> {
        self.stack_of_open_elements.last().copied()
    }

    /// Get the parent node for insertion.
    fn insertion_location(&self) -> NodeId {
        self.current_node().unwrap_or(NodeId::ROOT)
    }

    /// Create attributes map from token attributes.
    fn attributes_to_map(attributes: &[Attribute]) -> AttributesMap {
        attributes
            .iter()
            .map(|attr| (attr.name.clone(), attr.value.clone()))
            .collect()
    }

    /// Create a new element node and return its NodeId.
    fn create_element(&mut self, tag_name: &str, attributes: &[Attribute]) -> NodeId {
        self.tree.alloc(NodeType::Element(ElementData {
            tag_name: tag_name.to_string(),
            attrs: Self::attributes_to_map(attributes),
        }))
    }

    /// Create a text node and return its NodeId.
    fn create_text_node(&mut self, data: String) -> NodeId {
        self.tree.alloc(NodeType::Text(data))
    }

    /// Create a comment node and return its NodeId.
    fn create_comment_node(&mut self, data: String) -> NodeId {
        self.tree.alloc(NodeType::Comment(data))
    }

    /// Add a child to a parent node.
    fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.tree.append_child(parent_id, child_id);
    }

    /// [§ 13.2.6.1 Insert a character](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-character)
    fn insert_character(&mut self, c: char) {
        let parent_id = self.insertion_location();

        // "If there is a Text node immediately before the adjusted insertion
        // location, then append data to that Text node's data."
        if let Some(&last_child_id) = self.tree.children(parent_id).last() {
            if let Some(arena_node) = self.tree.get_mut(last_child_id) {
                if let NodeType::Text(ref mut text_data) = arena_node.node_type {
                    text_data.push(c);
                    return;
                }
            }
        }

        // Otherwise, create a new text node
        let text_id = self.create_text_node(c.to_string());
        self.append_child(parent_id, text_id);
    }

    /// Insert a comment node at the current insertion location.
    fn insert_comment(&mut self, data: &str) {
        let parent_id = self.insertion_location();
        let comment_id = self.create_comment_node(data.to_string());
        self.append_child(parent_id, comment_id);
    }

    /// Insert a comment as the last child of the document.
    fn insert_comment_to_document(&mut self, data: &str) {
        let comment_id = self.create_comment_node(data.to_string());
        self.append_child(NodeId::ROOT, comment_id);
    }

    /// [§ 13.2.6.1 Insert an HTML element](https://html.spec.whatwg.org/multipage/parsing.html#insert-an-html-element)
    fn insert_html_element(&mut self, token: &Token) -> NodeId {
        if let Token::StartTag { name, attributes, .. } = token {
            let element_id = self.create_element(name, attributes);
            let parent_id = self.insertion_location();
            self.append_child(parent_id, element_id);
            self.stack_of_open_elements.push(element_id);
            element_id
        } else {
            panic!("insert_html_element called with non-StartTag token");
        }
    }

    /// Get the tag name of a node.
    fn get_tag_name(&self, id: NodeId) -> Option<&str> {
        self.tree.as_element(id).map(|data| data.tag_name.as_str())
    }

    /// Pop elements from the stack until we find one with the given tag name.
    fn pop_until_tag(&mut self, tag_name: &str) {
        while let Some(id) = self.stack_of_open_elements.pop() {
            if self.get_tag_name(id) == Some(tag_name) {
                break;
            }
        }
    }

    /// Pop elements until one of the given tag names is found.
    /// Used for heading elements where any h1-h6 can close any other.
    fn pop_until_one_of(&mut self, tag_names: &[&str]) {
        while let Some(idx) = self.stack_of_open_elements.pop() {
            if let Some(name) = self.get_tag_name(idx) {
                if tag_names.contains(&name) {
                    break;
                }
            }
        }
    }

    /// Check if an element with the given tag name is in scope.
    /// Simplified version - just checks if it's on the stack.
    fn has_element_in_scope(&self, tag_name: &str) -> bool {
        self.stack_of_open_elements
            .iter()
            .any(|&idx| self.get_tag_name(idx) == Some(tag_name))
    }

    /// [§ 13.2.6.4.7 "in body" - implicit end tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    /// Close an open element if present on the stack.
    /// Used for elements like <li>, <p>, <dd>, <dt> that implicitly close.
    fn close_element_if_in_scope(&mut self, tag_name: &str) {
        if self.has_element_in_scope(tag_name) {
            self.pop_until_tag(tag_name);
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
            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

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
            Token::StartTag { name, attributes, .. } if name == "html" => {
                let html_idx = self.create_element(name, attributes);
                self.append_child(NodeId::ROOT, html_idx);
                self.stack_of_open_elements.push(html_idx);
                self.insertion_mode = InsertionMode::BeforeHead;
            }

            // "An end tag whose tag name is one of: "head", "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. }
                if matches!(name.as_str(), "head" | "body" | "html" | "br") =>
            {
                self.handle_before_html_anything_else(token);
            }

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_before_html_anything_else(token);
            }
        }
    }

    /// "Anything else" branch for "before html" mode:
    /// "Create an html element whose node document is the Document object. Append it to the
    /// Document object. Put this element in the stack of open elements."
    /// ...
    /// "Switch the insertion mode to "before head", then reprocess the token."
    fn handle_before_html_anything_else(&mut self, token: &Token) {
        let html_idx = self.create_element("html", &[]);
        self.append_child(NodeId::ROOT, html_idx);
        self.stack_of_open_elements.push(html_idx);
        self.insertion_mode = InsertionMode::BeforeHead;
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

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

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

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_before_head_anything_else(token);
            }
        }
    }

    /// "Anything else" branch for "before head" mode:
    /// "Insert an HTML element for a "head" start tag token with no attributes."
    /// "Set the head element pointer to the newly created head element."
    /// "Switch the insertion mode to "in head"."
    /// "Reprocess the current token."
    fn handle_before_head_anything_else(&mut self, token: &Token) {
        let head_idx = self.create_element("head", &[]);
        let parent_idx = self.insertion_location();
        self.append_child(parent_idx, head_idx);
        self.stack_of_open_elements.push(head_idx);
        self.head_element_pointer = Some(head_idx);
        self.insertion_mode = InsertionMode::InHead;
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

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

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
                if matches!(name.as_str(), "base" | "basefont" | "bgsound" | "link" | "meta") =>
            {
                self.insert_html_element(token);
                self.stack_of_open_elements.pop();
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
                self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode.clone());
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
                self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode.clone());
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
                self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode.clone());
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The tokenizer handles switching to ScriptData state for script elements
            }

            // "An end tag whose tag name is "head""
            // "Pop the current node (which will be the head element) off the stack of open elements."
            // "Switch the insertion mode to "after head"."
            Token::EndTag { name, .. } if name == "head" => {
                self.stack_of_open_elements.pop();
                self.insertion_mode = InsertionMode::AfterHead;
            }

            // "An end tag whose tag name is one of: "body", "html", "br""
            // "Act as described in the "anything else" entry below."
            Token::EndTag { name, .. } if matches!(name.as_str(), "body" | "html" | "br") => {
                self.handle_in_head_anything_else(token);
            }

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_in_head_anything_else(token);
            }
        }
    }

    /// "Anything else" branch for "in head" mode:
    /// "Pop the current node (which will be the head element) off the stack of open elements."
    /// "Switch the insertion mode to "after head"."
    /// "Reprocess the token."
    fn handle_in_head_anything_else(&mut self, token: &Token) {
        self.stack_of_open_elements.pop();
        self.insertion_mode = InsertionMode::AfterHead;
        self.reprocess_token(token);
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
                self.stack_of_open_elements.pop();
                self.insertion_mode = self.original_insertion_mode.unwrap_or(InsertionMode::InBody);
                // NOTE: Spec says to reprocess, but EOF is terminal so we just switch mode.
            }

            // "An end tag whose tag name is "script""
            // (Complex script handling - not implemented)
            //
            // "Any other end tag"
            // "Pop the current node off the stack of open elements."
            // "Switch the insertion mode to the original insertion mode."
            Token::EndTag { .. } => {
                self.stack_of_open_elements.pop();
                self.insertion_mode = self.original_insertion_mode.unwrap_or(InsertionMode::InBody);
            }

            // NOTE: Start tags and other tokens should not appear in text mode
            // per the tokenizer's behavior. If they do, it indicates a bug.
            _ => {
                panic!(
                    "Unexpected token in Text mode: {:?}. This indicates a tokenizer or parser bug.",
                    token
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

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

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
                self.insert_html_element(token);
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

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

            // "Anything else"
            _ => {
                self.handle_after_head_anything_else(token);
            }
        }
    }

    /// "Anything else" branch for "after head" mode:
    /// "Insert an HTML element for a "body" start tag token with no attributes."
    /// "Switch the insertion mode to "in body"."
    /// "Reprocess the current token."
    fn handle_after_head_anything_else(&mut self, token: &Token) {
        let body_idx = self.create_element("body", &[]);
        let parent_idx = self.insertion_location();
        self.append_child(parent_idx, body_idx);
        self.stack_of_open_elements.push(body_idx);
        self.insertion_mode = InsertionMode::InBody;
        self.reprocess_token(token);
    }

    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// NOTE: This is a partial implementation. The full spec has many more cases
    /// for formatting elements, adoption agency algorithm, etc.
    fn handle_in_body_mode(&mut self, token: &Token) {
        match token {
            // "A character token that is U+0000 NULL"
            // "Parse error. Ignore the token."
            Token::Character { data: '\0' } => {}

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
                // NOTE: We skip "reconstruct the active formatting elements" as we don't
                // implement the list of active formatting elements.
                self.insert_character(*data);
            }

            // "A comment token"
            // "Insert a comment."
            Token::Comment { data } => {
                self.insert_comment(data);
            }

            // "A DOCTYPE token"
            // "Parse error. Ignore the token."
            Token::Doctype { .. } => {}

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
                self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "p"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "p""
            // "If the stack of open elements has a p element in button scope, then close a p element."
            // "Insert an HTML element for the token."
            Token::StartTag { name, .. } if name == "p" => {
                // Close any existing <p> element first
                self.close_element_if_in_scope("p");
                self.insert_html_element(token);
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
                self.insert_html_element(token);
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
                if let Some(idx) = self.current_node() {
                    if let Some(tag) = self.get_tag_name(idx) {
                        if matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                            self.stack_of_open_elements.pop();
                        }
                    }
                }
                self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "a"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "a""
            // ... complex adoption agency handling for nested <a> tags ...
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Push onto the list of active formatting elements that element."
            Token::StartTag { name, .. } if name == "a" => {
                // NOTE: We skip the adoption agency algorithm for nested <a> tags
                // and the list of active formatting elements for simplicity
                self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tags for formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "b", "big", "code", "em", "font", "i",
            //  "s", "small", "strike", "strong", "tt", "u""
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Push onto the list of active formatting elements that element."
            //
            // "A start tag whose tag name is "nobr""
            // "Reconstruct the active formatting elements, if any."
            // "If the stack of open elements has a nobr element in scope, then this is a parse error;
            //  run the adoption agency algorithm for the token, then once again reconstruct the
            //  active formatting elements, if any."
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
                        | "span"
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
                // NOTE: We skip the list of active formatting elements and adoption agency for simplicity
                self.insert_html_element(token);
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
            //  ...
            //  8. Done: If the stack of open elements has a p element in button scope, then close a p element.
            //  9. Insert an HTML element for the token."
            Token::StartTag { name, .. } if name == "li" => {
                // Close any existing <li> element first
                self.close_element_if_in_scope("li");
                // Close any <p> in button scope
                self.close_element_if_in_scope("p");
                self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "dd", "dt"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // Similar to <li> but checks for dd/dt
            Token::StartTag { name, .. } if matches!(name.as_str(), "dd" | "dt") => {
                // Close any existing <dd> or <dt> element
                self.close_element_if_in_scope("dd");
                self.close_element_if_in_scope("dt");
                self.close_element_if_in_scope("p");
                self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - End tags "dd", "dt", "li"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            Token::EndTag { name, .. } if matches!(name.as_str(), "dd" | "dt" | "li") => {
                self.pop_until_tag(name);
            }

            // [§ 13.2.6.4.7 "in body" - Start tag "table"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is "table""
            // "If the Document is not set to quirks mode, and the stack of open elements has a p
            //  element in button scope, then close a p element."
            // "Insert an HTML element for the token."
            // "Set the frameset-ok flag to "not ok"."
            // "Switch the insertion mode to "in table"."
            //
            // NOTE: We simplify by not switching to InTable mode - just insert the element
            // and continue in InBody mode. This means table elements won't have proper
            // foster parenting behavior, but basic tables will render.
            Token::StartTag { name, .. } if name == "table" => {
                self.close_element_if_in_scope("p");
                self.insert_html_element(token);
            }

            // Table-related start tags: tr, td, th, tbody, thead, tfoot, caption, colgroup, col
            // Per spec these should only appear in InTable/InTableBody/InRow modes, but
            // for simplified parsing we just insert them as elements.
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" | "caption" | "colgroup" | "col"
                ) =>
            {
                self.insert_html_element(token);
                // col and colgroup are void-like in tables
                if matches!(name.as_str(), "col") {
                    self.stack_of_open_elements.pop();
                }
            }

            // Table-related end tags
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "table" | "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" | "caption"
                        | "colgroup"
                ) =>
            {
                self.pop_until_tag(name);
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
                self.insert_html_element(token);
                self.stack_of_open_elements.pop();
            }

            // [§ 13.2.6.4.7 "in body"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // "A start tag whose tag name is one of: "base", "basefont", "bgsound", "link",
            // "meta", "noframes", "script", "style", "template", "title""
            // "Process the token using the rules for the "in head" insertion mode."
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script"
                        | "style" | "template" | "title"
                ) =>
            {
                self.handle_in_head_mode(token);
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
                // NOTE: We skip scope checking and implied end tag generation for simplicity.
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
                // NOTE: We skip the scope check for simplicity
                self.pop_until_tag("p");
            }

            // [§ 13.2.6.4.7 "in body" - Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // Handle end tags for inline elements like span, a, em, strong, etc.
            // The spec uses the "Adoption Agency Algorithm" for formatting elements,
            // but for simplicity we just pop until the matching tag.
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "span" | "a" | "b" | "i" | "em" | "strong" | "small" | "s" | "cite"
                        | "q" | "dfn" | "abbr" | "ruby" | "rt" | "rp" | "data" | "time"
                        | "code" | "var" | "samp" | "kbd" | "sub" | "sup" | "u" | "mark"
                        | "bdi" | "bdo" | "wbr" | "nobr"
                ) =>
            {
                // NOTE: This is a simplified version - the spec uses the Adoption Agency Algorithm
                // for formatting elements, which is more complex.
                self.pop_until_tag(name);
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

            // Unhandled tokens - panic to surface missing implementations
            _ => {
                if let Token::StartTag { name, .. } = token {
                    todo!(
                        "Unhandled start tag <{}> in InBody mode - implement handler",
                        name
                    );
                } else if let Token::EndTag { name, .. } = token {
                    todo!(
                        "Unhandled end tag </{}> in InBody mode - implement handler",
                        name
                    );
                } else {
                    panic!(
                        "Unexpected token in InBody mode: {:?}. This indicates a parser bug.",
                        token
                    );
                }
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
                println!("{}Document", prefix);
            }
            NodeType::Element(data) => {
                if data.attrs.is_empty() {
                    println!("{}<{}>", prefix, data.tag_name);
                } else {
                    let attrs: Vec<String> = data
                        .attrs
                        .iter()
                        .map(|(k, v)| {
                            if v.is_empty() {
                                k.clone()
                            } else {
                                format!("{}=\"{}\"", k, v)
                            }
                        })
                        .collect();
                    println!("{}<{} {}>", prefix, data.tag_name, attrs.join(" "));
                }
            }
            NodeType::Text(data) => {
                let display = data.replace('\n', "\\n").replace(' ', "\u{00B7}");
                println!("{}\"{}\"", prefix, display);
            }
            NodeType::Comment(data) => {
                println!("{}<!-- {} -->", prefix, data);
            }
        }
        for &child_id in tree.children(id) {
            print_tree(tree, child_id, indent + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib_dom::Node;
    use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

    /// Helper to parse HTML and return the DOM tree
    fn parse(html: &str) -> DomTree {
        let mut tokenizer = HTMLTokenizer::new(html.to_string());
        tokenizer.run();
        let parser = HTMLParser::new(tokenizer.into_tokens());
        parser.run()
    }

    /// Helper to get element by tag name (first match, depth-first)
    fn find_element(tree: &DomTree, from: NodeId, tag: &str) -> Option<NodeId> {
        if let Some(data) = tree.as_element(from) {
            if data.tag_name == tag {
                return Some(from);
            }
        }
        for &child_id in tree.children(from) {
            if let Some(found) = find_element(tree, child_id, tag) {
                return Some(found);
            }
        }
        None
    }

    /// Helper to get text content of a node (concatenated)
    fn text_content(tree: &DomTree, id: NodeId) -> String {
        let mut result = String::new();
        if let Some(node) = tree.get(id) {
            match &node.node_type {
                NodeType::Text(data) => result.push_str(data),
                _ => {
                    for &child_id in tree.children(id) {
                        result.push_str(&text_content(tree, child_id));
                    }
                }
            }
        }
        result
    }

    /// Helper to get a node reference
    fn get_node(tree: &DomTree, id: NodeId) -> &Node {
        tree.get(id).expect("Node not found")
    }

    #[test]
    fn test_document_structure() {
        let tree = parse("<!DOCTYPE html><html><head></head><body></body></html>");

        // Root should be Document
        let root = get_node(&tree, NodeId::ROOT);
        assert!(matches!(root.node_type, NodeType::Document));

        // Document should have html child
        let html_id = find_element(&tree, NodeId::ROOT, "html");
        assert!(html_id.is_some());

        // html should have head and body
        let html_id = html_id.unwrap();
        let head_id = find_element(&tree, html_id, "head");
        let body_id = find_element(&tree, html_id, "body");
        assert!(head_id.is_some());
        assert!(body_id.is_some());
    }

    #[test]
    fn test_text_node() {
        let tree = parse("<html><body>Hello World</body></html>");
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

        let text = text_content(&tree, body_id);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_comment_node() {
        let tree = parse("<html><body><!-- test comment --></body></html>");
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

        // Body should have a comment child
        let has_comment = tree.children(body_id).iter().any(|&child_id| {
            if let Some(node) = tree.get(child_id) {
                matches!(&node.node_type, NodeType::Comment(data) if data == " test comment ")
            } else {
                false
            }
        });
        assert!(has_comment);
    }

    #[test]
    fn test_nested_elements() {
        let tree = parse("<html><body><div><p>Text</p></div></body></html>");

        let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
        let p_id = find_element(&tree, div_id, "p").unwrap();
        let text = text_content(&tree, p_id);

        assert_eq!(text, "Text");
    }

    #[test]
    fn test_element_attributes() {
        let tree = parse(r#"<html><body><div id="main" class="container"></div></body></html>"#);
        let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
        let div = get_node(&tree, div_id);

        if let NodeType::Element(data) = &div.node_type {
            assert_eq!(data.attrs.get("id"), Some(&"main".to_string()));
            assert_eq!(data.attrs.get("class"), Some(&"container".to_string()));
        } else {
            panic!("Expected Element");
        }
    }

    #[test]
    fn test_void_elements() {
        let tree = parse(r#"<html><body><input type="text"><br></body></html>"#);
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

        // Both input and br should be children of body (void elements don't nest)
        let element_names: Vec<_> = tree
            .children(body_id)
            .iter()
            .filter_map(|&child_id| {
                tree.as_element(child_id).map(|data| data.tag_name.as_str())
            })
            .collect();

        assert!(element_names.contains(&"input"));
        assert!(element_names.contains(&"br"));
    }

    #[test]
    fn test_title_element() {
        let tree = parse("<html><head><title>My Page</title></head><body></body></html>");
        let title_id = find_element(&tree, NodeId::ROOT, "title").unwrap();
        let text = text_content(&tree, title_id);

        assert_eq!(text, "My Page");
    }

    #[test]
    fn test_meta_element() {
        let tree = parse(r#"<html><head><meta charset="UTF-8"></head><body></body></html>"#);
        let meta_id = find_element(&tree, NodeId::ROOT, "meta").unwrap();
        let meta = get_node(&tree, meta_id);

        if let NodeType::Element(data) = &meta.node_type {
            assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
        } else {
            panic!("Expected Element");
        }
    }

    #[test]
    fn test_whitespace_preserved_in_text() {
        let tree = parse("<html><body>  hello  world  </body></html>");
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();
        let text = text_content(&tree, body_id);

        // Whitespace should be preserved
        assert_eq!(text, "  hello  world  ");
    }

    #[test]
    fn test_multiple_text_nodes_merged() {
        // Adjacent character tokens should become a single text node
        let tree = parse("<html><body>abc</body></html>");
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

        // Should have exactly one text node child (merged from a, b, c)
        let text_nodes: Vec<_> = tree
            .children(body_id)
            .iter()
            .filter(|&&child_id| {
                tree.get(child_id)
                    .map(|n| matches!(n.node_type, NodeType::Text(_)))
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(text_nodes.len(), 1);
        assert_eq!(text_content(&tree, body_id), "abc");
    }

    #[test]
    fn test_simple_html_file() {
        // Test parsing of the actual simple.html structure
        let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <!-- This is a comment -->
    <title>Test</title>
</head>
<body class="main" id="content">
    <!-- TODO: add more content -->
    <div data-value='single quoted'>Hello</div>
    <input type="text" disabled />
</body>
</html>"#;

        let tree = parse(html);

        // Check basic structure
        let root = get_node(&tree, NodeId::ROOT);
        assert!(matches!(root.node_type, NodeType::Document));

        let html_id = find_element(&tree, NodeId::ROOT, "html").unwrap();
        let html_elem = get_node(&tree, html_id);
        if let NodeType::Element(data) = &html_elem.node_type {
            assert_eq!(data.attrs.get("lang"), Some(&"en".to_string()));
        }

        // Check head elements
        let title_id = find_element(&tree, NodeId::ROOT, "title").unwrap();
        assert_eq!(text_content(&tree, title_id), "Test");

        let meta_id = find_element(&tree, NodeId::ROOT, "meta").unwrap();
        let meta = get_node(&tree, meta_id);
        if let NodeType::Element(data) = &meta.node_type {
            assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
        }

        // Check body elements
        let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();
        let body = get_node(&tree, body_id);
        if let NodeType::Element(data) = &body.node_type {
            assert_eq!(data.attrs.get("class"), Some(&"main".to_string()));
            assert_eq!(data.attrs.get("id"), Some(&"content".to_string()));
        }

        // Check div with single-quoted attribute
        let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
        let div = get_node(&tree, div_id);
        if let NodeType::Element(data) = &div.node_type {
            assert_eq!(data.attrs.get("data-value"), Some(&"single quoted".to_string()));
        }
        assert_eq!(text_content(&tree, div_id), "Hello");

        // Check input with boolean attribute
        let input_id = find_element(&tree, NodeId::ROOT, "input").unwrap();
        let input = get_node(&tree, input_id);
        if let NodeType::Element(data) = &input.node_type {
            assert_eq!(data.attrs.get("type"), Some(&"text".to_string()));
            assert_eq!(data.attrs.get("disabled"), Some(&"".to_string()));
        }
    }

    // ========== Raw text element tests at parser level ==========

    #[test]
    fn test_style_element_content_preserved() {
        // Style content should be preserved as text, not parsed as HTML
        let html = r#"<!DOCTYPE html>
<html>
<head>
<style>
body { color: red; }
.container { margin: 0; }
</style>
</head>
<body></body>
</html>"#;

        let tree = parse(html);
        let style = find_element(&tree, tree.root(), "style").unwrap();
        let content = text_content(&tree, style);

        // The CSS should be preserved as text
        assert!(content.contains("body { color: red; }"));
        assert!(content.contains(".container { margin: 0; }"));
    }

    #[test]
    fn test_style_with_html_like_content() {
        // HTML-like content inside style should NOT be interpreted as tags
        let html = "<html><head><style><div>not a tag</div></style></head><body></body></html>";

        let tree = parse(html);
        let style = find_element(&tree, tree.root(), "style").unwrap();
        let content = text_content(&tree, style);

        // The <div> should appear as literal text
        assert_eq!(content, "<div>not a tag</div>");

        // There should be no div element in the document (since it's inside style)
        let body = find_element(&tree, tree.root(), "body").unwrap();
        let div_in_body = find_element(&tree, body, "div");
        assert!(div_in_body.is_none());
    }

    #[test]
    fn test_title_content_preserved() {
        let html = "<html><head><title>My <test> Title</title></head><body></body></html>";

        let tree = parse(html);
        let title = find_element(&tree, tree.root(), "title").unwrap();
        let content = text_content(&tree, title);

        // Title content including < should be preserved
        assert_eq!(content, "My <test> Title");
    }
}
