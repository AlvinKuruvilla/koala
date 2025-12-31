use strum_macros::Display;

use crate::lib_dom::{AttributesMap, ElementData, Node, NodeType};
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

/// Internal node representation during parsing.
/// Uses indices for children to enable arena-style allocation.
#[derive(Debug, Clone)]
struct ParserNode {
    node_type: NodeType,
    children: Vec<usize>, // Indices into the nodes arena
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
    /// Stores indices into the nodes arena.
    stack_of_open_elements: Vec<usize>,

    /// [§ 13.2.4.4 The element pointers](https://html.spec.whatwg.org/multipage/parsing.html#the-element-pointers)
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

    /// [§ 13.2.6.1 Insert a character](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-character)
    fn insert_character(&mut self, c: char) {
        let parent_idx = self.insertion_location();

        // "If there is a Text node immediately before the adjusted insertion
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

    /// [§ 13.2.6.1 Insert an HTML element](https://html.spec.whatwg.org/multipage/parsing.html#insert-an-html-element)
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
                self.append_child(0, html_idx);
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
        self.append_child(0, html_idx);
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
                self.original_insertion_mode = Some(InsertionMode::InHead);
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
                self.original_insertion_mode = Some(InsertionMode::InHead);
                self.insertion_mode = InsertionMode::Text;
                // NOTE: The tokenizer handles switching to RAWTEXT state for these elements
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
            // per the tokenizer's behavior, but we ignore them if they do.
            _ => {}
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
                        | "p"
                        | "search"
                        | "section"
                        | "summary"
                        | "ul"
                ) =>
            {
                // NOTE: We skip "close a p element" check for simplicity.
                self.insert_html_element(token);
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
                        | "bdi" | "bdo" | "wbr"
                ) =>
            {
                // NOTE: This is a simplified version - the spec uses the Adoption Agency Algorithm
                // for formatting elements, which is more complex.
                self.pop_until_tag(name);
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

            // Default: insert if it's a start tag
            // NOTE: This is a simplified catch-all. The full spec has many more specific cases.
            _ => {
                if let Token::StartTag { .. } = token {
                    self.insert_html_element(token);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

    /// Helper to parse HTML and return the document node
    fn parse(html: &str) -> Node {
        let mut tokenizer = HTMLTokenizer::new(html.to_string());
        tokenizer.run();
        let parser = HTMLParser::new(tokenizer.into_tokens());
        parser.run()
    }

    /// Helper to get element by tag name (first match, depth-first)
    fn find_element<'a>(node: &'a Node, tag: &str) -> Option<&'a Node> {
        if let NodeType::Element(data) = &node.node_type {
            if data.tag_name == tag {
                return Some(node);
            }
        }
        for child in &node.children {
            if let Some(found) = find_element(child, tag) {
                return Some(found);
            }
        }
        None
    }

    /// Helper to get text content of a node (concatenated)
    fn text_content(node: &Node) -> String {
        let mut result = String::new();
        match &node.node_type {
            NodeType::Text(data) => result.push_str(data),
            _ => {
                for child in &node.children {
                    result.push_str(&text_content(child));
                }
            }
        }
        result
    }

    #[test]
    fn test_document_structure() {
        let doc = parse("<!DOCTYPE html><html><head></head><body></body></html>");

        // Root should be Document
        assert!(matches!(doc.node_type, NodeType::Document));

        // Document should have html child
        let html = find_element(&doc, "html");
        assert!(html.is_some());

        // html should have head and body
        let html = html.unwrap();
        let head = find_element(html, "head");
        let body = find_element(html, "body");
        assert!(head.is_some());
        assert!(body.is_some());
    }

    #[test]
    fn test_text_node() {
        let doc = parse("<html><body>Hello World</body></html>");
        let body = find_element(&doc, "body").unwrap();

        let text = text_content(body);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_comment_node() {
        let doc = parse("<html><body><!-- test comment --></body></html>");
        let body = find_element(&doc, "body").unwrap();

        // Body should have a comment child
        let has_comment = body.children.iter().any(|child| {
            matches!(&child.node_type, NodeType::Comment(data) if data == " test comment ")
        });
        assert!(has_comment);
    }

    #[test]
    fn test_nested_elements() {
        let doc = parse("<html><body><div><p>Text</p></div></body></html>");

        let div = find_element(&doc, "div").unwrap();
        let p = find_element(div, "p").unwrap();
        let text = text_content(p);

        assert_eq!(text, "Text");
    }

    #[test]
    fn test_element_attributes() {
        let doc = parse(r#"<html><body><div id="main" class="container"></div></body></html>"#);
        let div = find_element(&doc, "div").unwrap();

        if let NodeType::Element(data) = &div.node_type {
            assert_eq!(data.attrs.get("id"), Some(&"main".to_string()));
            assert_eq!(data.attrs.get("class"), Some(&"container".to_string()));
        } else {
            panic!("Expected Element");
        }
    }

    #[test]
    fn test_void_elements() {
        let doc = parse(r#"<html><body><input type="text"><br></body></html>"#);
        let body = find_element(&doc, "body").unwrap();

        // Both input and br should be children of body (void elements don't nest)
        let element_names: Vec<_> = body
            .children
            .iter()
            .filter_map(|child| {
                if let NodeType::Element(data) = &child.node_type {
                    Some(data.tag_name.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert!(element_names.contains(&"input"));
        assert!(element_names.contains(&"br"));
    }

    #[test]
    fn test_title_element() {
        let doc = parse("<html><head><title>My Page</title></head><body></body></html>");
        let title = find_element(&doc, "title").unwrap();
        let text = text_content(title);

        assert_eq!(text, "My Page");
    }

    #[test]
    fn test_meta_element() {
        let doc = parse(r#"<html><head><meta charset="UTF-8"></head><body></body></html>"#);
        let meta = find_element(&doc, "meta").unwrap();

        if let NodeType::Element(data) = &meta.node_type {
            assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
        } else {
            panic!("Expected Element");
        }
    }

    #[test]
    fn test_whitespace_preserved_in_text() {
        let doc = parse("<html><body>  hello  world  </body></html>");
        let text = text_content(find_element(&doc, "body").unwrap());

        // Whitespace should be preserved
        assert_eq!(text, "  hello  world  ");
    }

    #[test]
    fn test_multiple_text_nodes_merged() {
        // Adjacent character tokens should become a single text node
        let doc = parse("<html><body>abc</body></html>");
        let body = find_element(&doc, "body").unwrap();

        // Should have exactly one text node child (merged from a, b, c)
        let text_nodes: Vec<_> = body
            .children
            .iter()
            .filter(|child| matches!(child.node_type, NodeType::Text(_)))
            .collect();

        assert_eq!(text_nodes.len(), 1);
        assert_eq!(text_content(body), "abc");
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

        let doc = parse(html);

        // Check basic structure
        assert!(matches!(doc.node_type, NodeType::Document));

        let html_elem = find_element(&doc, "html").unwrap();
        if let NodeType::Element(data) = &html_elem.node_type {
            assert_eq!(data.attrs.get("lang"), Some(&"en".to_string()));
        }

        // Check head elements
        let title = find_element(&doc, "title").unwrap();
        assert_eq!(text_content(title), "Test");

        let meta = find_element(&doc, "meta").unwrap();
        if let NodeType::Element(data) = &meta.node_type {
            assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
        }

        // Check body elements
        let body = find_element(&doc, "body").unwrap();
        if let NodeType::Element(data) = &body.node_type {
            assert_eq!(data.attrs.get("class"), Some(&"main".to_string()));
            assert_eq!(data.attrs.get("id"), Some(&"content".to_string()));
        }

        // Check div with single-quoted attribute
        let div = find_element(&doc, "div").unwrap();
        if let NodeType::Element(data) = &div.node_type {
            assert_eq!(data.attrs.get("data-value"), Some(&"single quoted".to_string()));
        }
        assert_eq!(text_content(div), "Hello");

        // Check input with boolean attribute
        let input = find_element(&doc, "input").unwrap();
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

        let doc = parse(html);
        let style = find_element(&doc, "style").unwrap();
        let content = text_content(style);

        // The CSS should be preserved as text
        assert!(content.contains("body { color: red; }"));
        assert!(content.contains(".container { margin: 0; }"));
    }

    #[test]
    fn test_style_with_html_like_content() {
        // HTML-like content inside style should NOT be interpreted as tags
        let html = "<html><head><style><div>not a tag</div></style></head><body></body></html>";

        let doc = parse(html);
        let style = find_element(&doc, "style").unwrap();
        let content = text_content(style);

        // The <div> should appear as literal text
        assert_eq!(content, "<div>not a tag</div>");

        // There should be no div element in the document (since it's inside style)
        let div_in_body = find_element(find_element(&doc, "body").unwrap(), "div");
        assert!(div_in_body.is_none());
    }

    #[test]
    fn test_title_content_preserved() {
        let html = "<html><head><title>My <test> Title</title></head><body></body></html>";

        let doc = parse(html);
        let title = find_element(&doc, "title").unwrap();
        let content = text_content(title);

        // Title content including < should be preserved
        assert_eq!(content, "My <test> Title");
    }
}
