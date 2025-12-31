use core::fmt;

// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
// "The output of the tokenization step is a series of zero or more of the following
// tokens: DOCTYPE, start tag, end tag, comment, character, end-of-file."

/// An attribute on a start or end tag token.
/// Spec: "a list of attributes, each of which has a name and a value"
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

impl Attribute {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

/// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
/// The tokenizer emits tokens of these types to the tree construction stage.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "DOCTYPE tokens have a name, a public identifier, a system identifier,
    // and a force-quirks flag. When a DOCTYPE token is created, its name,
    // public identifier, and system identifier must be marked as missing
    // (which is a distinct state from the empty string), and the force-quirks
    // flag must be set to off (its other state is on)."
    Doctype {
        name: Option<String>,
        public_identifier: Option<String>,
        system_identifier: Option<String>,
        force_quirks: bool,
    },

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "Start and end tag tokens have a tag name, a self-closing flag, and a
    // list of attributes, each of which has a name and a value. When a start
    // or end tag token is created, its self-closing flag must be unset (its
    // other state is that it be set), and its attributes list must be empty."
    StartTag {
        name: String,
        self_closing: bool,
        attributes: Vec<Attribute>,
    },

    EndTag {
        name: String,
        attributes: Vec<Attribute>,
    },

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "Comment and character tokens have data."
    Comment { data: String },

    Character { data: char },

    EndOfFile,
}

impl Token {
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "When a DOCTYPE token is created, its name, public identifier, and system
    // identifier must be marked as missing (which is a distinct state from the
    // empty string), and the force-quirks flag must be set to off."
    pub fn new_doctype() -> Self {
        Token::Doctype {
            name: None,
            public_identifier: None,
            system_identifier: None,
            force_quirks: false,
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "When a start or end tag token is created, its self-closing flag must be
    // unset (its other state is that it be set), and its attributes list must
    // be empty."
    pub fn new_start_tag() -> Self {
        Token::StartTag {
            name: String::new(),
            self_closing: false,
            attributes: Vec::new(),
        }
    }

    pub fn new_end_tag() -> Self {
        Token::EndTag {
            name: String::new(),
            attributes: Vec::new(),
        }
    }

    pub fn new_comment() -> Self {
        Token::Comment {
            data: String::new(),
        }
    }

    pub fn new_character(c: char) -> Self {
        Token::Character { data: c }
    }

    pub fn new_eof() -> Self {
        Token::EndOfFile
    }

    /// Returns true if this is an end-of-file token.
    pub fn is_eof(&self) -> bool {
        matches!(self, Token::EndOfFile)
    }

    // -------------------------------------------------------------------------
    // Mutation helpers for use during tokenization.
    // These panic if called on the wrong token variant, which indicates a bug
    // in the tokenizer state machine.
    // -------------------------------------------------------------------------

    /// Append a character to the DOCTYPE token's name.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#doctype-name-state
    /// "Append the current input character to the current DOCTYPE token's name."
    pub fn append_to_doctype_name(&mut self, c: char) {
        match self {
            Token::Doctype { name, .. } => {
                if let Some(ref mut n) = name {
                    n.push(c);
                } else {
                    *name = Some(c.to_string());
                }
            }
            _ => panic!("append_to_doctype_name called on non-DOCTYPE token"),
        }
    }

    /// Append a character to a start or end tag token's name.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state
    /// "Append the current input character to the current tag token's tag name."
    pub fn append_to_tag_name(&mut self, c: char) {
        match self {
            Token::StartTag { name, .. } | Token::EndTag { name, .. } => {
                name.push(c);
            }
            _ => panic!("append_to_tag_name called on non-tag token"),
        }
    }

    /// Set the self-closing flag on a start tag token.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#self-closing-start-tag-state
    /// "Set the self-closing flag of the current tag token."
    pub fn set_self_closing(&mut self) {
        match self {
            Token::StartTag { self_closing, .. } => {
                *self_closing = true;
            }
            _ => panic!("set_self_closing called on non-start-tag token"),
        }
    }

    /// Append a character to a comment token's data.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-state
    /// "Append the current input character to the comment token's data."
    pub fn append_to_comment(&mut self, c: char) {
        match self {
            Token::Comment { data } => {
                data.push(c);
            }
            _ => panic!("append_to_comment called on non-comment token"),
        }
    }

    /// Set the force-quirks flag on a DOCTYPE token.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-name-state
    /// "Set the current DOCTYPE token's force-quirks flag to on."
    pub fn set_force_quirks(&mut self) {
        match self {
            Token::Doctype { force_quirks, .. } => {
                *force_quirks = true;
            }
            _ => panic!("set_force_quirks called on non-DOCTYPE token"),
        }
    }

    // -------------------------------------------------------------------------
    // Attribute mutation helpers
    // -------------------------------------------------------------------------

    /// Start a new attribute with empty name and value.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state
    /// "Start a new attribute in the current tag token."
    pub fn start_new_attribute(&mut self) {
        match self {
            Token::StartTag { attributes, .. } | Token::EndTag { attributes, .. } => {
                attributes.push(Attribute::new(String::new(), String::new()));
            }
            _ => panic!("start_new_attribute called on non-tag token"),
        }
    }

    /// Append a character to the current attribute's name.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state
    /// "Append the current input character to the current attribute's name."
    pub fn append_to_current_attribute_name(&mut self, c: char) {
        match self {
            Token::StartTag { attributes, .. } | Token::EndTag { attributes, .. } => {
                if let Some(attr) = attributes.last_mut() {
                    attr.name.push(c);
                }
            }
            _ => panic!("append_to_current_attribute_name called on non-tag token"),
        }
    }

    /// Append a character to the current attribute's value.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state
    /// "Append the current input character to the current attribute's value."
    pub fn append_to_current_attribute_value(&mut self, c: char) {
        match self {
            Token::StartTag { attributes, .. } | Token::EndTag { attributes, .. } => {
                if let Some(attr) = attributes.last_mut() {
                    attr.value.push(c);
                }
            }
            _ => panic!("append_to_current_attribute_value called on non-tag token"),
        }
    }

    /// Check if the current attribute name is a duplicate of an existing attribute.
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state
    /// "When the user agent leaves the attribute name state (and before emitting the
    /// tag token, if appropriate), the complete attribute's name must be compared to
    /// the other attributes on the same token; if there is already an attribute on
    /// the token with the exact same name, then this is a duplicate-attribute parse
    /// error and the new attribute must be removed from the token."
    pub fn current_attribute_name_is_duplicate(&self) -> bool {
        match self {
            Token::StartTag { attributes, .. } | Token::EndTag { attributes, .. } => {
                if let Some(current) = attributes.last() {
                    // Check if any other attribute has the same name
                    attributes[..attributes.len() - 1]
                        .iter()
                        .any(|attr| attr.name == current.name)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Remove the current (last) attribute from the token.
    /// Used when a duplicate attribute is detected.
    pub fn remove_current_attribute(&mut self) {
        match self {
            Token::StartTag { attributes, .. } | Token::EndTag { attributes, .. } => {
                attributes.pop();
            }
            _ => panic!("remove_current_attribute called on non-tag token"),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Doctype {
                name,
                public_identifier,
                system_identifier,
                force_quirks,
            } => {
                write!(f, "DOCTYPE")?;
                if let Some(n) = name {
                    write!(f, " {}", n)?;
                }
                if let Some(pub_id) = public_identifier {
                    write!(f, " PUBLIC \"{}\"", pub_id)?;
                }
                if let Some(sys_id) = system_identifier {
                    write!(f, " SYSTEM \"{}\"", sys_id)?;
                }
                if *force_quirks {
                    write!(f, " (force-quirks)")?;
                }
                Ok(())
            }
            Token::StartTag {
                name,
                self_closing,
                attributes,
            } => {
                write!(f, "<{}", name)?;
                for attr in attributes {
                    write!(f, " {}=\"{}\"", attr.name, attr.value)?;
                }
                if *self_closing {
                    write!(f, " /")?;
                }
                write!(f, ">")
            }
            Token::EndTag { name, .. } => {
                write!(f, "</{}>", name)
            }
            Token::Comment { data } => {
                write!(f, "<!--{}-->", data)
            }
            Token::Character { data } => {
                // Show whitespace characters explicitly
                match data {
                    '\n' => write!(f, "Character(\\n)"),
                    '\t' => write!(f, "Character(\\t)"),
                    ' ' => write!(f, "Character(SPACE)"),
                    c => write!(f, "Character({})", c),
                }
            }
            Token::EndOfFile => write!(f, "EOF"),
        }
    }
}
