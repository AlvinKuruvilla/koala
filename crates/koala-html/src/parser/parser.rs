use strum_macros::Display;

use koala_common::warning::warn_once;
use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};

use super::foreign_content::{
    adjust_foreign_attributes, adjust_mathml_attributes, adjust_svg_attributes,
};
use crate::tokenizer::{Attribute, Token};

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
        /// The NodeId of the element in the DOM tree.
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
    /// Stores NodeIds into the arena.
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
            active_formatting_elements: Vec::new(),
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
    ///
    /// Logs via koala-common's warning system and stores the issue for later retrieval.
    fn parse_warning(&mut self, message: &str) {
        warn_once("HTML Parser", message);
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
            InsertionMode::InHeadNoscript => {
                // TODO: [§ 13.2.6.4.5](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inheadnoscript)
                self.handle_in_head_noscript_mode(token)
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
            // TODO: Implement table parsing in this order:
            //
            // STEP 1: InTable mode - handles <table>, <caption>, <colgroup>, <tbody>, <tr>
            //   [§ 13.2.6.4.9](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable)
            //   - "A start tag whose tag name is 'caption'" -> push, switch to InCaption
            //   - "A start tag whose tag name is 'colgroup'" -> switch to InColumnGroup
            //   - "A start tag whose tag name is one of: 'tbody', 'tfoot', 'thead'" -> switch to InTableBody
            //   - Foster parenting for misplaced content
            InsertionMode::InTable => todo!("InTable mode - see STEP 1 above"),

            // STEP 2: InTableText mode - accumulates character tokens in table context
            //   [§ 13.2.6.4.10](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext)
            //   - Accumulate character tokens
            //   - On other token: flush characters (foster parent if non-whitespace)
            InsertionMode::InTableText => todo!("InTableText mode - see STEP 2 above"),

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
            InsertionMode::InTableBody => todo!("InTableBody mode - see STEP 5 above"),

            // STEP 6: InRow mode - handles <tr> and its children
            //   [§ 13.2.6.4.14](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inrow)
            //   - "A start tag whose tag name is one of: 'th', 'td'" -> switch to InCell
            InsertionMode::InRow => todo!("InRow mode - see STEP 6 above"),

            // STEP 7: InCell mode - handles <td> and <th> content
            //   [§ 13.2.6.4.15](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incell)
            //   - Process most tokens using InBody rules
            //   - Special handling for table-related end tags
            InsertionMode::InCell => todo!("InCell mode - see STEP 7 above"),

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
    fn is_whitespace(c: char) -> bool {
        matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ')
    }

    /// [§ 13.2.4.3 The stack of open elements](https://html.spec.whatwg.org/multipage/parsing.html#current-node)
    ///
    /// "The current node is the bottommost node in this stack of open elements."
    fn current_node(&self) -> Option<NodeId> {
        self.stack_of_open_elements.last().copied()
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)
    ///
    /// "The adjusted insertion location is the current node, if the stack
    /// of open elements is not empty."
    ///
    /// NOTE: This is a simplified version. The full algorithm handles
    /// foster parenting for table elements.
    fn insertion_location(&self) -> NodeId {
        self.current_node().unwrap_or(NodeId::ROOT)
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)
    ///
    /// Convert token attributes to the AttributesMap used by ElementData.
    fn attributes_to_map(attributes: &[Attribute]) -> AttributesMap {
        attributes
            .iter()
            .map(|attr| (attr.name.clone(), attr.value.clone()))
            .collect()
    }

    /// [§ 13.2.6.1 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#create-an-element-for-the-token)
    ///
    /// "Create an element for a token"
    ///
    /// Creates a new element node in the DOM arena.
    /// NOTE: This is a simplified version; full algorithm handles namespaces,
    /// custom elements, and the "will execute script" flag.
    fn create_element(&mut self, tag_name: &str, attributes: &[Attribute]) -> NodeId {
        self.tree.alloc(NodeType::Element(ElementData {
            tag_name: tag_name.to_string(),
            attrs: Self::attributes_to_map(attributes),
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
        let parent_id = self.insertion_location();

        // STEP 2: "If there is a Text node immediately before the adjusted
        //         insertion location, then append data to that Text node's data."
        if let Some(&last_child_id) = self.tree.children(parent_id).last() {
            if let Some(arena_node) = self.tree.get_mut(last_child_id) {
                if let NodeType::Text(ref mut text_data) = arena_node.node_type {
                    text_data.push(c);
                    return;
                }
            }
        }

        // STEP 3: "Otherwise, create a new Text node whose data is data and
        //         whose node document is the same as that of the element in
        //         which the adjusted insertion location finds itself, and
        //         insert the newly created node at the adjusted insertion location."
        let text_id = self.create_text_node(c.to_string());
        self.append_child(parent_id, text_id);
    }

    /// [§ 13.2.6.1 Insert a comment](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-comment)
    ///
    /// "When the steps below require the user agent to insert a comment
    /// while processing a comment token, optionally with an explicitly
    /// insertion position position..."
    fn insert_comment(&mut self, data: &str) {
        // STEP 1: "Let the adjusted insertion location be the appropriate place
        //         for inserting a node."
        let parent_id = self.insertion_location();
        // STEP 2: "Create a Comment node..."
        let comment_id = self.create_comment_node(data.to_string());
        // STEP 3: "Insert the newly created node at the adjusted insertion location."
        self.append_child(parent_id, comment_id);
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
    /// "When the steps below require the user agent to insert an HTML element
    /// for a token, the user agent must insert a foreign element for the token,
    /// in the HTML namespace."
    fn insert_html_element(&mut self, token: &Token) -> NodeId {
        if let Token::StartTag {
            name, attributes, ..
        } = token
        {
            // STEP 1: "Create an element for the token"
            let element_id = self.create_element(name, attributes);

            // STEP 2: "Let the adjusted insertion location be the appropriate
            //         place for inserting a node."
            let parent_id = self.insertion_location();

            // STEP 3: "Append the new element to the node at the adjusted
            //         insertion location."
            self.append_child(parent_id, element_id);

            // STEP 4: "Push the element onto the stack of open elements."
            self.stack_of_open_elements.push(element_id);

            element_id
        } else {
            panic!("insert_html_element called with non-StartTag token");
        }
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
    ///
    /// The scope markers for "has an element in scope" (default scope) are:
    /// - applet, caption, html, table, td, th, marquee, object, template
    /// - MathML: mi, mo, mn, ms, mtext, annotation-xml
    /// - SVG: foreignObject, desc, title
    ///
    /// Other scope types add additional markers:
    /// - "has an element in list item scope": adds ol, ul
    /// - "has an element in button scope": adds button
    /// - "has an element in table scope": html, table, template only
    /// - "has an element in select scope": optgroup, option only (inverted)
    ///
    /// NOTE: Current implementation is simplified - checks if element exists
    /// anywhere on stack without respecting scope boundaries. Full implementation
    /// would require checking against scope marker lists above.
    fn has_element_in_scope(&self, tag_name: &str) -> bool {
        // TODO: Implement proper scope checking algorithm:
        //
        // STEP 1: Initialize node to be the current node (bottommost on stack).
        //         let mut node_index = self.stack_of_open_elements.len() - 1;
        //
        // STEP 2: Loop:
        //         let node = self.stack_of_open_elements[node_index];
        //         let node_tag = self.get_tag_name(node);
        //
        //         // STEP 2a: If node is the target, return true (match state)
        //         if node_tag == Some(tag_name) {
        //             return true;
        //         }
        //
        //         // STEP 2b: If node is a scope marker, return false (failure state)
        //         // TODO: Check against scope marker list:
        //         //   DEFAULT_SCOPE_MARKERS = ["applet", "caption", "html", "table",
        //         //     "td", "th", "marquee", "object", "template",
        //         //     // MathML: "mi", "mo", "mn", "ms", "mtext", "annotation-xml",
        //         //     // SVG: "foreignObject", "desc", "title"
        //         //   ];
        //         // if DEFAULT_SCOPE_MARKERS.contains(&node_tag) {
        //         //     return false;
        //         // }
        //
        //         // STEP 2c: Move to previous node and continue loop
        //         if node_index == 0 { break; }
        //         node_index -= 1;
        //
        // Current simplified implementation: just check if element exists anywhere
        self.stack_of_open_elements
            .iter()
            .any(|&idx| self.get_tag_name(idx) == Some(tag_name))
    }

    /// [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    ///
    /// This helper combines two spec operations commonly used together:
    ///
    /// [§ 13.2.6.2 Generate implied end tags](https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags)
    /// "While the current node is a dd, dt, li, optgroup, option, p, rb, rp, rt,
    ///  or rtc element, the UA must pop the current node off the stack."
    ///
    /// Then: Check if element is in scope and pop until found.
    ///
    /// Used for elements like <li>, <p>, <dd>, <dt> that implicitly close
    /// when a new one is encountered.
    ///
    /// NOTE: Current implementation skips "generate implied end tags" step.
    /// Full implementation would first pop dd/dt/li/optgroup/option/p/rb/rp/rt/rtc.
    fn close_element_if_in_scope(&mut self, tag_name: &str) {
        // TODO: STEP 1: Generate implied end tags
        // [§ 13.2.6.2](https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags)
        //
        // const IMPLIED_END_TAG_ELEMENTS: &[&str] = &[
        //     "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc"
        // ];
        //
        // while let Some(&current) = self.stack_of_open_elements.last() {
        //     if let Some(tag) = self.get_tag_name(current) {
        //         if IMPLIED_END_TAG_ELEMENTS.contains(&tag) && tag != tag_name {
        //             self.stack_of_open_elements.pop();
        //             continue;
        //         }
        //     }
        //     break;
        // }

        // STEP 2: Check if element is in scope
        if self.has_element_in_scope(tag_name) {
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
            Token::StartTag {
                name, attributes, ..
            } if name == "html" => {
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

    /// [§ 13.2.6.4.2 The "before html" insertion mode - Anything else](https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode)
    ///
    /// "Anything else":
    /// "Create an html element whose node document is the Document object.
    /// Append it to the Document object. Put this element in the stack of
    /// open elements. Switch the insertion mode to "before head", then
    /// reprocess the token."
    fn handle_before_html_anything_else(&mut self, token: &Token) {
        // STEP 1: "Create an html element whose node document is the Document object."
        let html_idx = self.create_element("html", &[]);

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

    /// [§ 13.2.6.4.3 The "before head" insertion mode - Anything else](https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode)
    ///
    /// "Anything else":
    /// "Insert an HTML element for a "head" start tag token with no attributes.
    /// Set the head element pointer to the newly created head element.
    /// Switch the insertion mode to "in head". Reprocess the current token."
    fn handle_before_head_anything_else(&mut self, token: &Token) {
        // STEP 1: "Insert an HTML element for a 'head' start tag token with no attributes."
        let head_idx = self.create_element("head", &[]);
        let parent_idx = self.insertion_location();
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
                let _ = self.insert_html_element(token);
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
                let _ = self.insert_html_element(token);
                // "Let the original insertion mode be the current insertion mode."
                self.original_insertion_mode = Some(self.insertion_mode.clone());
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

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

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
        // A DOCTYPE token
        match token {
            Token::Doctype { .. } => {
                // TODO: Parse error. Ignore the token.
            }
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
            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::StartTag { name, .. } if matches!(name.as_str(), "head" | "noscript") => {
                // Parse error. Ignore the token.
            }
            Token::EndTag { .. } => {
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

            // "Any other end tag"
            // "Parse error. Ignore the token."
            Token::EndTag { .. } => {}

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
        let body_idx = self.create_element("body", &[]);
        let parent_idx = self.insertion_location();
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
                self.reconstruct_active_formatting_elements();
                self.insert_character(*data);
                // TODO: Set the frameset-ok flag to "not ok
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
                if let Some(idx) = self.current_node() {
                    if let Some(tag) = self.get_tag_name(idx) {
                        if matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                            let _ = self.stack_of_open_elements.pop();
                        }
                    }
                }
                let _ = self.insert_html_element(token);
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
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Formatting element start tags](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is one of: "b", "big", "code", "em", "font", "i",
            //  "s", "small", "strike", "strong", "tt", "u""
            //
            // "Reconstruct the active formatting elements, if any."
            // "Insert an HTML element for the token."
            // "Push onto the list of active formatting elements that element."
            //
            // [§ 13.2.4.3 The list of active formatting elements](https://html.spec.whatwg.org/multipage/parsing.html#the-list-of-active-formatting-elements)
            //
            // TODO: Implement active formatting elements list:
            //
            // // Data structure to track formatting elements
            // enum ActiveFormattingEntry {
            //     Element { node_id: NodeId, token: Token },
            //     Marker,  // Inserted when entering applet, object, marquee, template, td, th, caption
            // }
            // active_formatting_elements: Vec<ActiveFormattingEntry>
            //
            // // Push formatting element onto list (called here)
            // fn push_active_formatting_element(&mut self, element: NodeId, token: &Token) {
            //     // STEP 1: If there are already 3 elements with same tag name, remove earliest
            //     // STEP 2: Push Element entry onto list
            //     self.active_formatting_elements.push(ActiveFormattingEntry::Element {
            //         node_id: element,
            //         token: token.clone(),
            //     });
            // }
            //
            // // Reconstruct active formatting elements (called before inserting content)
            // fn reconstruct_active_formatting_elements(&mut self) {
            //     // STEP 1: If list is empty, return
            //     // STEP 2: If last entry is marker or in stack of open elements, return
            //     // STEP 3: Let entry be the last element in the list
            //     // STEP 4: Rewind: If entry is first in list, jump to Create
            //     // STEP 5: Let entry = previous entry in list
            //     // STEP 6: If entry is not marker and not in stack, go to Rewind
            //     // STEP 7: Advance: entry = next entry in list
            //     // STEP 8: Create: insert element for entry's token, replace entry with new element
            //     // STEP 9: If entry is not last in list, go to Advance
            // }
            //
            // NOTE: Current implementation skips active formatting elements.
            // This means we don't handle implicit reopening of formatting across blocks.
            // Example that would render incorrectly:
            //   <p><b>bold<p>still bold</b>  -- second p should inherit bold
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
                // Simplified: just insert element without active formatting elements tracking
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
            //  ...
            //  8. Done: If the stack of open elements has a p element in button scope, then close a p element.
            //  9. Insert an HTML element for the token."
            Token::StartTag { name, .. } if name == "li" => {
                // Close any existing <li> element first
                self.close_element_if_in_scope("li");
                // Close any <p> in button scope
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
            }

            // [§ 13.2.6.4.7 "in body" - Start tags "dd", "dt"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // Similar to <li> but checks for dd/dt
            Token::StartTag { name, .. } if matches!(name.as_str(), "dd" | "dt") => {
                // Close any existing <dd> or <dt> element
                self.close_element_if_in_scope("dd");
                self.close_element_if_in_scope("dt");
                self.close_element_if_in_scope("p");
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
                todo!("Insert marker at end of active formatting elements list");
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
                if let Some(&node_id) = self.stack_of_open_elements.last() {
                    if self.get_tag_name(node_id) == Some("option") {
                        let _ = self.stack_of_open_elements.pop();
                    }
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
            // TODO: Implement proper table parsing with InTable mode and foster parenting:
            //
            // [§ 13.2.6.1 Foster parenting](https://html.spec.whatwg.org/multipage/parsing.html#foster-parent)
            //
            // Foster parenting handles content that appears inside <table> but outside
            // proper table structure (e.g., text directly in <table>). Such content
            // must be "foster parented" - inserted before the table instead.
            //
            // fn get_foster_parent(&self) -> (NodeId, InsertPosition) {
            //     // STEP 1: Let last table be the last table element in stack of open elements
            //     // STEP 2: If there is a last table:
            //     //   - If last table has a parent, foster parent is parent, insert before table
            //     //   - Otherwise, foster parent is element immediately above table in stack
            //     // STEP 3: If there is no last table, foster parent is first element in stack (html)
            // }
            //
            // // When inserting in InTable mode and current node is not table-compatible:
            // fn insert_foster_parented(&mut self, node: NodeId) {
            //     let (parent, position) = self.get_foster_parent();
            //     // Insert node at position instead of as child of current node
            // }
            //
            // Also requires implementing insertion modes:
            // - InTable: handles table, caption, colgroup, col, tbody, thead, tfoot, tr
            // - InTableBody: handles tbody, thead, tfoot, tr content
            // - InRow: handles tr, td, th content
            // - InCell: handles td, th content
            //
            // NOTE: Current simplified implementation just inserts table elements normally.
            // This means invalid content like <table>text</table> won't be foster parented.
            Token::StartTag { name, .. } if name == "table" => {
                self.close_element_if_in_scope("p");
                let _ = self.insert_html_element(token);
            }

            // Table-related start tags: tr, td, th, tbody, thead, tfoot, caption, colgroup, col
            // Per spec these should only appear in InTable/InTableBody/InRow modes, but
            // for simplified parsing we just insert them as elements.
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "tr" | "td"
                        | "th"
                        | "tbody"
                        | "thead"
                        | "tfoot"
                        | "caption"
                        | "colgroup"
                        | "col"
                ) =>
            {
                let _ = self.insert_html_element(token);
                // col and colgroup are void-like in tables
                if matches!(name.as_str(), "col") {
                    let _ = self.stack_of_open_elements.pop();
                }
            }

            // Table-related end tags
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "table"
                        | "tr"
                        | "td"
                        | "th"
                        | "tbody"
                        | "thead"
                        | "tfoot"
                        | "caption"
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
                    // TODO: generate_implied_end_tags() before popping
                    self.pop_until_tag(name);
                    todo!("Clear active formatting elements up to last marker");
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

            // [§ 13.2.6.4.7 "in body" - Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // For formatting elements, the spec uses the Adoption Agency Algorithm:
            // [§ 13.2.6.4.7 Adoption agency algorithm](https://html.spec.whatwg.org/multipage/parsing.html#adoption-agency-algorithm)
            //
            // The algorithm handles misnested formatting like: <b>text<i>more</b>text</i>
            // by "adopting" nodes between the formatting element and the misnested end tag.
            //
            // TODO: Implement full adoption agency algorithm:
            //
            // fn run_adoption_agency(&mut self, tag_name: &str) {
            //     // STEP 1: Let outer loop counter be 0.
            //     let mut outer_loop_counter = 0;
            //
            //     loop {
            //         // STEP 2: If outer loop counter >= 8, return.
            //         if outer_loop_counter >= 8 { return; }
            //
            //         // STEP 3: Increment outer loop counter.
            //         outer_loop_counter += 1;
            //
            //         // STEP 4: Let formatting element be the last element in the list of
            //         //         active formatting elements that:
            //         //         - has the same tag name as the token
            //         //         - is between the end of the list and the last marker
            //         // let formatting_element = self.active_formatting_elements
            //         //     .iter().rev()
            //         //     .take_while(|e| !e.is_marker())
            //         //     .find(|e| e.tag_name == tag_name);
            //
            //         // STEP 5: If there is no such element, return (use "any other end tag" steps)
            //         // if formatting_element.is_none() { return self.any_other_end_tag(tag_name); }
            //
            //         // STEP 6: If formatting element is not in stack of open elements, remove
            //         //         from active formatting elements and return.
            //
            //         // STEP 7: If formatting element is in stack but not in scope, parse error, return.
            //
            //         // STEP 8: If formatting element is not the current node, parse error (continue).
            //
            //         // STEP 9: Let furthest block be the topmost element in the stack that is
            //         //         lower than formatting element AND is in the "special" category.
            //         //         Special elements: address, applet, area, article, aside, base, ...
            //
            //         // STEP 10: If there is no furthest block, pop until formatting element,
            //         //          remove from active formatting elements, return.
            //
            //         // STEP 11-21: The complex reparenting loop:
            //         //   - Create bookmark at formatting element position
            //         //   - Let node = furthest block, last node = furthest block
            //         //   - Inner loop (up to 3 iterations):
            //         //     - Move node up the stack
            //         //     - If node is formatting element, break
            //         //     - If node is not in active formatting elements, remove from stack, continue
            //         //     - Create new element, replace in active formatting elements
            //         //     - Reparent last node to new element
            //         //   - Insert last node at appropriate place
            //         //   - Create new element for formatting element
            //         //   - Reparent furthest block's children to new element
            //         //   - Append new element to furthest block
            //         //   - Remove old formatting element, insert new at bookmark
            //     }
            // }
            //
            // Current simplified implementation - just pops until matching tag.
            // This works for properly nested content but won't handle misnesting correctly.
            Token::EndTag { name, .. }
                if matches!(
                    name.as_str(),
                    "span"
                        | "a"
                        | "b"
                        | "i"
                        | "em"
                        | "strong"
                        | "small"
                        | "s"
                        | "cite"
                        | "q"
                        | "dfn"
                        | "abbr"
                        | "ruby"
                        | "rt"
                        | "rp"
                        | "data"
                        | "time"
                        | "code"
                        | "var"
                        | "samp"
                        | "kbd"
                        | "sub"
                        | "sup"
                        | "u"
                        | "mark"
                        | "bdi"
                        | "bdo"
                        | "wbr"
                        | "nobr"
                        | "label"
                ) =>
            {
                // Simplified: just pop until matching tag (works for properly nested content)
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

            // ===== FOREIGN CONTENT (SVG and MathML) =====
            //
            // [§ 13.2.6.4.7 "in body" - A start tag whose tag name is "math"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            // [§ 13.2.6.4.7 "in body" - A start tag whose tag name is "svg"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "A start tag whose tag name is 'math'":
            // "A start tag whose tag name is 'svg'":
            //   "Reconstruct the active formatting elements, if any.
            //    Adjust MathML attributes for the token. (This fixes the case of MathML
            //    attributes that are not all lowercase.)
            //    Adjust foreign attributes for the token. (This fixes the use of namespaced
            //    attributes, in particular XLink.)
            //    Insert a foreign element for the token, in the [MathML/SVG] namespace.
            //    If the token has its self-closing flag set, pop the current node off the
            //    stack of open elements and acknowledge the token's self-closing flag."
            //
            // NOTE: Current implementation adjusts attributes per spec but treats the
            // element as HTML (no namespace). Full foreign content parsing (§ 13.2.6.5)
            // is not yet implemented.
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } if name == "svg" => {
                // STEP 1: Reconstruct the active formatting elements, if any.
                //   [§ 13.2.4.3](https://html.spec.whatwg.org/multipage/parsing.html#reconstruct-the-active-formatting-elements)
                self.reconstruct_active_formatting_elements();

                // STEP 2: Adjust attributes for foreign content
                //   [§ 13.2.6.3](https://html.spec.whatwg.org/multipage/parsing.html#adjust-svg-attributes)
                let mut adjusted_attributes = attributes.clone();
                adjust_svg_attributes(&mut adjusted_attributes);
                adjust_foreign_attributes(&mut adjusted_attributes);

                // STEP 3: Insert a foreign element for the token
                //   [§ 13.2.6.1](https://html.spec.whatwg.org/multipage/parsing.html#insert-a-foreign-element)
                //   NOTE: We insert as HTML element since our DOM doesn't support namespaces yet.
                //   Full implementation would use SVG namespace "http://www.w3.org/2000/svg"
                let adjusted_token = Token::StartTag {
                    name: name.clone(),
                    attributes: adjusted_attributes,
                    self_closing: *self_closing,
                };
                let element_id = self.insert_html_element(&adjusted_token);

                // STEP 4: Handle self-closing flag
                //   "If the token has its self-closing flag set, pop the current node off
                //    the stack of open elements and acknowledge the token's self-closing flag."
                if *self_closing {
                    let _ = self.stack_of_open_elements.pop();
                    // NOTE: Acknowledging the self-closing flag prevents a parse error.
                    // Since we don't track parse errors for this, we just pop.
                }

                // STEP 5: If not self-closing, future tokens should be processed by
                //   "in foreign content" rules (§ 13.2.6.5). This is not yet implemented.
                //   For now, we continue processing as HTML which works for simple cases.
                let _ = element_id;
            }

            Token::StartTag {
                name,
                attributes,
                self_closing,
            } if name == "math" => {
                // STEP 1: Reconstruct the active formatting elements, if any.
                self.reconstruct_active_formatting_elements();

                // STEP 2: Adjust attributes for foreign content
                //   [§ 13.2.6.3](https://html.spec.whatwg.org/multipage/parsing.html#adjust-mathml-attributes)
                let mut adjusted_attributes = attributes.clone();
                adjust_mathml_attributes(&mut adjusted_attributes);
                adjust_foreign_attributes(&mut adjusted_attributes);

                // STEP 3: Insert a foreign element for the token
                //   NOTE: We insert as HTML element since our DOM doesn't support namespaces yet.
                //   Full implementation would use MathML namespace "http://www.w3.org/1998/Math/MathML"
                let adjusted_token = Token::StartTag {
                    name: name.clone(),
                    attributes: adjusted_attributes,
                    self_closing: *self_closing,
                };
                let element_id = self.insert_html_element(&adjusted_token);

                // STEP 4: Handle self-closing flag
                if *self_closing {
                    let _ = self.stack_of_open_elements.pop();
                }

                let _ = element_id;
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
            Token::StartTag { name, .. } => {
                self.reconstruct_active_formatting_elements();
                let _ = self.insert_html_element(token);
                // Log unknown standard elements so we can add explicit handlers if needed
                if !name.contains('-') {
                    warn_once(
                        "HTML Parser",
                        &format!("using generic handler for <{}>", name),
                    );
                }
            }

            // [§ 13.2.6.4.7 "in body" - Any other end tag](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
            //
            // "Any other end tag"
            // 1. "Initialize node to be the current node (the bottommost node of the stack)."
            // 2. "Loop: If node is an HTML element with the same tag name as the token, then:"
            //    a. "Generate implied end tags, except for elements with the same tag name"
            //    b. "If node is not the current node, then this is a parse error."
            //    c. "Pop all the nodes from the current node up to node, including node, then stop."
            // 3. "Otherwise, if node is in the special category, parse error; ignore the token."
            // 4. "Set node to the previous entry in the stack of open elements."
            // 5. "Return to the step labeled loop."
            //
            // NOTE: Simplified implementation - we just pop up to and including the tag.
            // Full implementation would check special category at each step.
            Token::EndTag { name, .. } => {
                if self.has_element_in_scope(name) {
                    self.pop_until_tag(name);
                }
                // Otherwise: parse error, ignore the token
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
