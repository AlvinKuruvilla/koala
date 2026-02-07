use core::fmt;

/// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
///
/// "The output of the tokenization step is a series of zero or more of the following
/// tokens: DOCTYPE, start tag, end tag, comment, character, end-of-file."
///
/// An attribute on a start or end tag token.
///
/// Per [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization):
/// "a list of attributes, each of which has a name and a value"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// "each of which has a name"
    pub name: String,
    /// "and a value"
    pub value: String,
}

impl Attribute {
    /// Create a new attribute with the given name and value.
    #[must_use]
    pub const fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

/// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
///
/// The tokenizer emits tokens of these types to the tree construction stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// "DOCTYPE tokens have a name, a public identifier, a system identifier,
    /// and a force-quirks flag. When a DOCTYPE token is created, its name,
    /// public identifier, and system identifier must be marked as missing
    /// (which is a distinct state from the empty string), and the force-quirks
    /// flag must be set to off (its other state is on)."
    Doctype {
        /// "a name"
        name: Option<String>,
        /// "a public identifier"
        public_identifier: Option<String>,
        /// "a system identifier"
        system_identifier: Option<String>,
        /// "a force-quirks flag"
        force_quirks: bool,
    },

    /// "Start and end tag tokens have a tag name, a self-closing flag, and a
    /// list of attributes, each of which has a name and a value. When a start
    /// or end tag token is created, its self-closing flag must be unset (its
    /// other state is that it be set), and its attributes list must be empty."
    StartTag {
        /// "a tag name"
        name: String,
        /// "a self-closing flag"
        self_closing: bool,
        /// "a list of attributes"
        attributes: Vec<Attribute>,
    },

    /// End tag token. Same structure as start tag per spec.
    EndTag {
        /// "a tag name"
        name: String,
        /// "a list of attributes"
        attributes: Vec<Attribute>,
    },

    /// "Comment and character tokens have data."
    Comment {
        /// "data"
        data: String,
    },

    /// "Comment and character tokens have data."
    Character {
        /// "data"
        data: char,
    },

    /// End-of-file token signals the end of input.
    EndOfFile,
}

impl Token {
    /// "When a DOCTYPE token is created, its name, public identifier, and system
    /// identifier must be marked as missing (which is a distinct state from the
    /// empty string), and the force-quirks flag must be set to off."
    #[must_use]
    pub const fn new_doctype() -> Self {
        Self::Doctype {
            name: None,
            public_identifier: None,
            system_identifier: None,
            force_quirks: false,
        }
    }

    /// "When a start or end tag token is created, its self-closing flag must be
    /// unset (its other state is that it be set), and its attributes list must
    /// be empty."
    #[must_use]
    pub const fn new_start_tag() -> Self {
        Self::StartTag {
            name: String::new(),
            self_closing: false,
            attributes: Vec::new(),
        }
    }

    /// Create a new end tag token per spec.
    #[must_use]
    pub const fn new_end_tag() -> Self {
        Self::EndTag {
            name: String::new(),
            attributes: Vec::new(),
        }
    }

    /// Create a new comment token with empty data.
    #[must_use]
    pub const fn new_comment() -> Self {
        Self::Comment {
            data: String::new(),
        }
    }

    /// Create a character token with the given character.
    #[must_use]
    pub const fn new_character(c: char) -> Self {
        Self::Character { data: c }
    }

    /// Create an end-of-file token.
    #[must_use]
    pub const fn new_eof() -> Self {
        Self::EndOfFile
    }

    /// Returns true if this is an end-of-file token.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::EndOfFile)
    }

    /// Mutation helpers for use during tokenization.
    /// These panic if called on the wrong token variant, which indicates a bug
    /// in the tokenizer state machine.
    ///
    /// [§ 13.2.5.55 DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-name-state)
    ///
    /// "Append the current input character to the current DOCTYPE token's name."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-DOCTYPE token, indicating a tokenizer bug.
    pub fn append_to_doctype_name(&mut self, c: char) {
        match self {
            Self::Doctype { name, .. } => {
                if let Some(n) = name {
                    n.push(c);
                } else {
                    *name = Some(String::from(c));
                }
            }
            _ => panic!("append_to_doctype_name called on non-DOCTYPE token"),
        }
    }

    /// [§ 13.2.5.8 Tag name state](https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state)
    ///
    /// "Append the current input character to the current tag token's tag name."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-tag token, indicating a tokenizer bug.
    pub fn append_to_tag_name(&mut self, c: char) {
        match self {
            Self::StartTag { name, .. } | Self::EndTag { name, .. } => {
                name.push(c);
            }
            _ => panic!("append_to_tag_name called on non-tag token"),
        }
    }

    /// [§ 13.2.5.40 Self-closing start tag state](https://html.spec.whatwg.org/multipage/parsing.html#self-closing-start-tag-state)
    ///
    /// "Set the self-closing flag of the current tag token."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-start-tag token, indicating a tokenizer bug.
    pub fn set_self_closing(&mut self) {
        match self {
            Self::StartTag { self_closing, .. } => {
                *self_closing = true;
            }
            _ => panic!("set_self_closing called on non-start-tag token"),
        }
    }

    /// [§ 13.2.5.45 Comment state](https://html.spec.whatwg.org/multipage/parsing.html#comment-state)
    ///
    /// "Append the current input character to the comment token's data."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-comment token, indicating a tokenizer bug.
    pub fn append_to_comment(&mut self, c: char) {
        match self {
            Self::Comment { data } => {
                data.push(c);
            }
            _ => panic!("append_to_comment called on non-comment token"),
        }
    }

    /// [§ 13.2.5.54 Before DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-name-state)
    ///
    /// "Set the current DOCTYPE token's force-quirks flag to on."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-DOCTYPE token, indicating a tokenizer bug.
    pub fn set_force_quirks(&mut self) {
        match self {
            Self::Doctype { force_quirks, .. } => {
                *force_quirks = true;
            }
            _ => panic!("set_force_quirks called on non-DOCTYPE token"),
        }
    }

    /// [§ 13.2.5.32 Before attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state)
    ///
    /// "Start a new attribute in the current tag token."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-tag token, indicating a tokenizer bug.
    pub fn start_new_attribute(&mut self) {
        match self {
            Self::StartTag { attributes, .. } | Self::EndTag { attributes, .. } => {
                attributes.push(Attribute::new(String::new(), String::new()));
            }
            _ => panic!("start_new_attribute called on non-tag token"),
        }
    }

    /// [§ 13.2.5.33 Attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state)
    ///
    /// "Append the current input character to the current attribute's name."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-tag token, indicating a tokenizer bug.
    pub fn append_to_current_attribute_name(&mut self, c: char) {
        match self {
            Self::StartTag { attributes, .. } | Self::EndTag { attributes, .. } => {
                if let Some(attr) = attributes.last_mut() {
                    attr.name.push(c);
                }
            }
            _ => panic!("append_to_current_attribute_name called on non-tag token"),
        }
    }

    /// [§ 13.2.5.36 Attribute value (double-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state)
    ///
    /// "Append the current input character to the current attribute's value."
    ///
    /// # Panics
    ///
    /// Panics if called on a non-tag token, indicating a tokenizer bug.
    pub fn append_to_current_attribute_value(&mut self, c: char) {
        match self {
            Self::StartTag { attributes, .. } | Self::EndTag { attributes, .. } => {
                if let Some(attr) = attributes.last_mut() {
                    attr.value.push(c);
                }
            }
            _ => panic!("append_to_current_attribute_value called on non-tag token"),
        }
    }

    /// [§ 13.2.5.33 Attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state)
    ///
    /// "When the user agent leaves the attribute name state (and before emitting the
    /// tag token, if appropriate), the complete attribute's name must be compared to
    /// the other attributes on the same token; if there is already an attribute on
    /// the token with the exact same name, then this is a duplicate-attribute parse
    /// error and the new attribute must be removed from the token."
    #[must_use]
    pub fn current_attribute_name_is_duplicate(&self) -> bool {
        match self {
            Self::StartTag { attributes, .. } | Self::EndTag { attributes, .. } => {
                attributes.last().is_some_and(|current| {
                    // Check if any other attribute has the same name
                    attributes[..attributes.len() - 1]
                        .iter()
                        .any(|attr| attr.name == current.name)
                })
            }
            _ => false,
        }
    }

    /// Remove the current (last) attribute from the token.
    /// Used when a duplicate attribute is detected.
    ///
    /// # Panics
    ///
    /// Panics if called on a non-tag token, indicating a tokenizer bug.
    pub fn remove_current_attribute(&mut self) {
        match self {
            Self::StartTag { attributes, .. } | Self::EndTag { attributes, .. } => {
                let _ = attributes.pop();
            }
            _ => panic!("remove_current_attribute called on non-tag token"),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Doctype {
                name,
                public_identifier,
                system_identifier,
                force_quirks,
            } => {
                write!(f, "DOCTYPE")?;
                if let Some(n) = name {
                    write!(f, " {n}")?;
                }
                if let Some(pub_id) = public_identifier {
                    write!(f, " PUBLIC \"{pub_id}\"")?;
                }
                if let Some(sys_id) = system_identifier {
                    write!(f, " SYSTEM \"{sys_id}\"")?;
                }
                if *force_quirks {
                    write!(f, " (force-quirks)")?;
                }
                Ok(())
            }
            Self::StartTag {
                name,
                self_closing,
                attributes,
            } => {
                write!(f, "<{name}")?;
                for attr in attributes {
                    write!(f, " {}=\"{}\"", attr.name, attr.value)?;
                }
                if *self_closing {
                    write!(f, " /")?;
                }
                write!(f, ">")
            }
            Self::EndTag { name, .. } => {
                write!(f, "</{name}>")
            }
            Self::Comment { data } => {
                write!(f, "<!--{data}-->")
            }
            Self::Character { data } => {
                // Show whitespace characters explicitly
                match data {
                    '\n' => write!(f, "Character(\\n)"),
                    '\t' => write!(f, "Character(\\t)"),
                    ' ' => write!(f, "Character(SPACE)"),
                    c => write!(f, "Character({c})"),
                }
            }
            Self::EndOfFile => write!(f, "EOF"),
        }
    }
}
