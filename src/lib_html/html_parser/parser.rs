use strum_macros::Display;

use crate::lib_dom::{AttributesMap, ElementData, Node, NodeType};
use crate::lib_html::html_tokenizer::token::{Attribute, Token};

/// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-insertion-mode
/// "The insertion mode is a state variable that controls the primary operation
/// of the tree construction stage."
#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum InsertionMode {
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode
    Initial,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode
    BeforeHtml,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode
    BeforeHead,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead
    InHead,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inheadnoscript
    InHeadNoscript,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode
    AfterHead,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
    InBody,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incdata
    Text,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable
    InTable,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext
    InTableText,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incaption
    InCaption,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incolumngroup
    InColumnGroup,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intablebody
    InTableBody,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow
    InRow,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incell
    InCell,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselect
    InSelect,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inselectintable
    InSelectInTable,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intemplate
    InTemplate,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterbody
    AfterBody,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inframeset
    InFrameset,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset
    AfterFrameset,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-body-insertion-mode
    AfterAfterBody,
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-frameset-insertion-mode
    AfterAfterFrameset,
}

/// Internal node representation during parsing.
/// Uses indices for children to enable arena-style allocation.
#[derive(Debug, Clone)]
struct ParserNode {
    node_type: NodeType,
    children: Vec<usize>, // Indices into the nodes arena
}

/// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tree-construction
/// The HTML parser builds a DOM tree from a stream of tokens.
pub struct HTMLParser {
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-insertion-mode
    insertion_mode: InsertionMode,

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#original-insertion-mode
    original_insertion_mode: Option<InsertionMode>,

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-stack-of-open-elements
    /// Stores indices into the nodes arena.
    stack_of_open_elements: Vec<usize>,

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-element-pointers
    head_element_pointer: Option<usize>,

    /// Arena of all nodes. Index 0 is the Document node.
    nodes: Vec<ParserNode>,

    /// Input tokens from the tokenizer.
    tokens: Vec<Token>,

    /// Current position in token stream.
    token_index: usize,

    /// Whether we've stopped parsing.
    stopped: bool,
}

impl HTMLParser {
    /// Create a new parser from a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        // Create the Document node at index 0
        let document = ParserNode {
            node_type: NodeType::Document,
            children: Vec::new(),
        };

        HTMLParser {
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: None,
            stack_of_open_elements: Vec::new(),
            head_element_pointer: None,
            nodes: vec![document],
            tokens,
            token_index: 0,
            stopped: false,
        }
    }

    /// Run the parser and return the document node.
    pub fn run(mut self) -> Node {
        while !self.stopped && self.token_index < self.tokens.len() {
            let token = self.tokens[self.token_index].clone();
            self.process_token(&token);
            self.token_index += 1;
        }

        // Convert arena to tree structure
        self.build_tree(0)
    }

    /// Recursively build a Node tree from the arena.
    fn build_tree(&self, index: usize) -> Node {
        let parser_node = &self.nodes[index];
        let children: Vec<Node> = parser_node
            .children
            .iter()
            .map(|&child_idx| self.build_tree(child_idx))
            .collect();

        Node {
            node_type: parser_node.node_type.clone(),
            children,
        }
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tree-construction-dispatcher
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

    // -------------------------------------------------------------------------
    // Helper functions
    // -------------------------------------------------------------------------

    fn is_whitespace(c: char) -> bool {
        matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ')
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#current-node
    fn current_node(&self) -> Option<usize> {
        self.stack_of_open_elements.last().copied()
    }

    /// Get the parent node for insertion.
    fn insertion_location(&self) -> usize {
        self.current_node().unwrap_or(0)
    }

    /// Create attributes map from token attributes.
    fn attributes_to_map(attributes: &[Attribute]) -> AttributesMap {
        attributes
            .iter()
            .map(|attr| (attr.name.clone(), attr.value.clone()))
            .collect()
    }

    /// Create a new element node and return its index.
    fn create_element(&mut self, tag_name: &str, attributes: &[Attribute]) -> usize {
        let node = ParserNode {
            node_type: NodeType::Element(ElementData {
                tag_name: tag_name.to_string(),
                attrs: Self::attributes_to_map(attributes),
            }),
            children: Vec::new(),
        };
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    /// Create a text node and return its index.
    fn create_text_node(&mut self, data: String) -> usize {
        let node = ParserNode {
            node_type: NodeType::Text(data),
            children: Vec::new(),
        };
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    /// Create a comment node and return its index.
    fn create_comment_node(&mut self, data: String) -> usize {
        let node = ParserNode {
            node_type: NodeType::Comment(data),
            children: Vec::new(),
        };
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    /// Add a child to a parent node.
    fn append_child(&mut self, parent_idx: usize, child_idx: usize) {
        self.nodes[parent_idx].children.push(child_idx);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#insert-a-character
    fn insert_character(&mut self, c: char) {
        let parent_idx = self.insertion_location();

        // Spec: "If there is a Text node immediately before the adjusted insertion
        // location, then append data to that Text node's data."
        if let Some(&last_child_idx) = self.nodes[parent_idx].children.last() {
            if let NodeType::Text(ref mut text_data) = self.nodes[last_child_idx].node_type {
                text_data.push(c);
                return;
            }
        }

        // Otherwise, create a new text node
        let text_idx = self.create_text_node(c.to_string());
        self.append_child(parent_idx, text_idx);
    }

    /// Insert a comment node at the current insertion location.
    fn insert_comment(&mut self, data: &str) {
        let parent_idx = self.insertion_location();
        let comment_idx = self.create_comment_node(data.to_string());
        self.append_child(parent_idx, comment_idx);
    }

    /// Insert a comment as the last child of the document.
    fn insert_comment_to_document(&mut self, data: &str) {
        let comment_idx = self.create_comment_node(data.to_string());
        self.append_child(0, comment_idx);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#insert-an-html-element
    fn insert_html_element(&mut self, token: &Token) -> usize {
        if let Token::StartTag { name, attributes, .. } = token {
            let element_idx = self.create_element(name, attributes);
            let parent_idx = self.insertion_location();
            self.append_child(parent_idx, element_idx);
            self.stack_of_open_elements.push(element_idx);
            element_idx
        } else {
            panic!("insert_html_element called with non-StartTag token");
        }
    }

    /// Get the tag name of a node.
    fn get_tag_name(&self, idx: usize) -> Option<&str> {
        if let NodeType::Element(ref data) = self.nodes[idx].node_type {
            Some(&data.tag_name)
        } else {
            None
        }
    }

    /// Pop elements from the stack until we find one with the given tag name.
    fn pop_until_tag(&mut self, tag_name: &str) {
        while let Some(idx) = self.stack_of_open_elements.pop() {
            if self.get_tag_name(idx) == Some(tag_name) {
                break;
            }
        }
    }

    // -------------------------------------------------------------------------
    // Insertion mode handlers
    // -------------------------------------------------------------------------

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode
    fn handle_initial_mode(&mut self, token: &Token) {
        match token {
            // Spec: "A character token that is one of [whitespace] - Ignore the token."
            Token::Character { data } if Self::is_whitespace(*data) => {}

            // Spec: "A comment token - Insert a comment as the last child of the Document."
            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            // Spec: "A DOCTYPE token - ... switch the insertion mode to 'before html'."
            Token::Doctype { .. } => {
                // Skip creating DocumentType node for simplicity
                self.insertion_mode = InsertionMode::BeforeHtml;
            }

            // Spec: "Anything else - switch to 'before html', reprocess."
            _ => {
                self.insertion_mode = InsertionMode::BeforeHtml;
                self.reprocess_token(token);
            }
        }
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode
    fn handle_before_html_mode(&mut self, token: &Token) {
        match token {
            Token::Doctype { .. } => {} // Parse error, ignore

            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            Token::Character { data } if Self::is_whitespace(*data) => {} // Ignore

            // Spec: "A start tag whose tag name is 'html'"
            Token::StartTag { name, attributes, .. } if name == "html" => {
                let html_idx = self.create_element(name, attributes);
                self.append_child(0, html_idx);
                self.stack_of_open_elements.push(html_idx);
                self.insertion_mode = InsertionMode::BeforeHead;
            }

            Token::EndTag { name, .. }
                if matches!(name.as_str(), "head" | "body" | "html" | "br") =>
            {
                self.handle_before_html_anything_else(token);
            }

            Token::EndTag { .. } => {} // Parse error, ignore

            _ => {
                self.handle_before_html_anything_else(token);
            }
        }
    }

    fn handle_before_html_anything_else(&mut self, token: &Token) {
        let html_idx = self.create_element("html", &[]);
        self.append_child(0, html_idx);
        self.stack_of_open_elements.push(html_idx);
        self.insertion_mode = InsertionMode::BeforeHead;
        self.reprocess_token(token);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode
    fn handle_before_head_mode(&mut self, token: &Token) {
        match token {
            Token::Character { data } if Self::is_whitespace(*data) => {} // Ignore

            Token::Comment { data } => {
                self.insert_comment(data);
            }

            Token::Doctype { .. } => {} // Parse error, ignore

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // Spec: "A start tag whose tag name is 'head'"
            Token::StartTag { name, .. } if name == "head" => {
                let head_idx = self.insert_html_element(token);
                self.head_element_pointer = Some(head_idx);
                self.insertion_mode = InsertionMode::InHead;
            }

            Token::EndTag { name, .. }
                if matches!(name.as_str(), "head" | "body" | "html" | "br") =>
            {
                self.handle_before_head_anything_else(token);
            }

            Token::EndTag { .. } => {} // Parse error, ignore

            _ => {
                self.handle_before_head_anything_else(token);
            }
        }
    }

    fn handle_before_head_anything_else(&mut self, token: &Token) {
        let head_idx = self.create_element("head", &[]);
        let parent_idx = self.insertion_location();
        self.append_child(parent_idx, head_idx);
        self.stack_of_open_elements.push(head_idx);
        self.head_element_pointer = Some(head_idx);
        self.insertion_mode = InsertionMode::InHead;
        self.reprocess_token(token);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead
    fn handle_in_head_mode(&mut self, token: &Token) {
        match token {
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.insert_character(*data);
            }

            Token::Comment { data } => {
                self.insert_comment(data);
            }

            Token::Doctype { .. } => {} // Parse error, ignore

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // Spec: Void elements in head
            Token::StartTag { name, .. }
                if matches!(name.as_str(), "base" | "basefont" | "bgsound" | "link" | "meta") =>
            {
                self.insert_html_element(token);
                self.stack_of_open_elements.pop();
            }

            // Spec: "A start tag whose tag name is 'title'"
            // Spec: "Follow the generic RCDATA element parsing algorithm."
            // https://html.spec.whatwg.org/multipage/parsing.html#generic-rcdata-element-parsing-algorithm
            Token::StartTag { name, .. } if name == "title" => {
                // Spec: "Insert an HTML element for the token."
                self.insert_html_element(token);
                // Spec: "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(InsertionMode::InHead);
                // Spec: "Switch the insertion mode to 'text'."
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The spec also says "Switch the tokenizer to the RCDATA state."
                // We don't have tokenizer integration, so we rely on the tokenizer
                // emitting character tokens that the Text mode will handle.
            }

            // Spec: "An end tag whose tag name is 'head'"
            Token::EndTag { name, .. } if name == "head" => {
                self.stack_of_open_elements.pop();
                self.insertion_mode = InsertionMode::AfterHead;
            }

            Token::EndTag { name, .. } if matches!(name.as_str(), "body" | "html" | "br") => {
                self.handle_in_head_anything_else(token);
            }

            Token::EndTag { .. } => {} // Parse error, ignore

            _ => {
                self.handle_in_head_anything_else(token);
            }
        }
    }

    fn handle_in_head_anything_else(&mut self, token: &Token) {
        self.stack_of_open_elements.pop();
        self.insertion_mode = InsertionMode::AfterHead;
        self.reprocess_token(token);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incdata
    /// "The 'text' insertion mode"
    fn handle_text_mode(&mut self, token: &Token) {
        match token {
            // Spec: "A character token - Insert the character."
            Token::Character { data } => {
                self.insert_character(*data);
            }

            // Spec: "An end-of-file token - Parse error. ... Pop the current node off
            // the stack of open elements. Switch the insertion mode to the original
            // insertion mode."
            Token::EndOfFile => {
                // Parse error
                self.stack_of_open_elements.pop();
                self.insertion_mode = self.original_insertion_mode.unwrap_or(InsertionMode::InBody);
            }

            // Spec: "Any other end tag - Pop the current node off the stack of open
            // elements. Switch the insertion mode to the original insertion mode."
            Token::EndTag { .. } => {
                self.stack_of_open_elements.pop();
                self.insertion_mode = self.original_insertion_mode.unwrap_or(InsertionMode::InBody);
            }

            // Ignore other tokens in text mode
            _ => {}
        }
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode
    fn handle_after_head_mode(&mut self, token: &Token) {
        match token {
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.insert_character(*data);
            }

            Token::Comment { data } => {
                self.insert_comment(data);
            }

            Token::Doctype { .. } => {} // Parse error, ignore

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // Spec: "A start tag whose tag name is 'body'"
            Token::StartTag { name, .. } if name == "body" => {
                self.insert_html_element(token);
                self.insertion_mode = InsertionMode::InBody;
            }

            Token::StartTag { name, .. } if name == "head" => {} // Parse error, ignore

            Token::EndTag { name, .. } if matches!(name.as_str(), "body" | "html" | "br") => {
                self.handle_after_head_anything_else(token);
            }

            Token::EndTag { .. } => {} // Parse error, ignore

            _ => {
                self.handle_after_head_anything_else(token);
            }
        }
    }

    fn handle_after_head_anything_else(&mut self, token: &Token) {
        let body_idx = self.create_element("body", &[]);
        let parent_idx = self.insertion_location();
        self.append_child(parent_idx, body_idx);
        self.stack_of_open_elements.push(body_idx);
        self.insertion_mode = InsertionMode::InBody;
        self.reprocess_token(token);
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
    fn handle_in_body_mode(&mut self, token: &Token) {
        match token {
            Token::Character { data: '\0' } => {} // Parse error, ignore

            Token::Character { data } => {
                self.insert_character(*data);
            }

            Token::Comment { data } => {
                self.insert_comment(data);
            }

            Token::Doctype { .. } => {} // Parse error, ignore

            Token::StartTag { name, .. } if name == "html" => {
                // Parse error. Merge attributes (simplified: ignore)
            }

            // Block elements
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
                        | "p"
                        | "search"
                        | "section"
                        | "summary"
                        | "ul"
                ) =>
            {
                self.insert_html_element(token);
            }

            // Void elements
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "area" | "br" | "embed" | "img" | "keygen" | "wbr" | "input" | "hr"
                ) =>
            {
                self.insert_html_element(token);
                self.stack_of_open_elements.pop();
            }

            // End tags for block elements
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
                self.pop_until_tag(name);
            }

            // Spec: "An end tag whose tag name is 'body'"
            Token::EndTag { name, .. } if name == "body" => {
                self.insertion_mode = InsertionMode::AfterBody;
            }

            // Spec: "An end tag whose tag name is 'html'"
            Token::EndTag { name, .. } if name == "html" => {
                self.insertion_mode = InsertionMode::AfterBody;
                self.reprocess_token(token);
            }

            Token::EndOfFile => {
                self.stopped = true;
            }

            // Default: insert if it's a start tag
            _ => {
                if let Token::StartTag { .. } = token {
                    self.insert_html_element(token);
                }
            }
        }
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterbody
    fn handle_after_body_mode(&mut self, token: &Token) {
        match token {
            Token::Character { data } if Self::is_whitespace(*data) => {
                self.handle_in_body_mode(token);
            }

            Token::Comment { data } => {
                // Insert as last child of html element
                if let Some(&html_idx) = self.stack_of_open_elements.first() {
                    let comment_idx = self.create_comment_node(data.clone());
                    self.append_child(html_idx, comment_idx);
                }
            }

            Token::Doctype { .. } => {} // Parse error, ignore

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            // Spec: "An end tag whose tag name is 'html'"
            Token::EndTag { name, .. } if name == "html" => {
                self.insertion_mode = InsertionMode::AfterAfterBody;
            }

            Token::EndOfFile => {
                self.stopped = true;
            }

            _ => {
                self.insertion_mode = InsertionMode::InBody;
                self.reprocess_token(token);
            }
        }
    }

    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#the-after-after-body-insertion-mode
    fn handle_after_after_body_mode(&mut self, token: &Token) {
        match token {
            Token::Comment { data } => {
                self.insert_comment_to_document(data);
            }

            Token::Doctype { .. } | Token::Character { data: _ }
                if matches!(token, Token::Character { data } if Self::is_whitespace(*data)) =>
            {
                self.handle_in_body_mode(token);
            }

            Token::StartTag { name, .. } if name == "html" => {
                self.handle_in_body_mode(token);
            }

            Token::EndOfFile => {
                self.stopped = true;
            }

            _ => {
                self.insertion_mode = InsertionMode::InBody;
                self.reprocess_token(token);
            }
        }
    }
}

/// Print a DOM tree for debugging.
pub fn print_tree(node: &Node, indent: usize) {
    let prefix = "  ".repeat(indent);
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
    for child in &node.children {
        print_tree(child, indent + 1);
    }
}
