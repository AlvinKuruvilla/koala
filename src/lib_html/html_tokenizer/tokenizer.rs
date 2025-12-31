use strum_macros::Display;

use super::token::Token;

#[derive(Debug, PartialEq, Display)]
pub enum TokenizerState {
    Data,
    RCDATA,
    RAWTEXT,
    ScriptData,
    PLAINTEXT,
    TagOpen,
    EndTagOpen,
    TagName,
    RCDATALessThanSign,
    RCDATAEndTagOpen,
    RCDATAEndTagName,
    RAWTEXTLessThanSign,
    RAWTEXTEndTagOpen,
    RAWTEXTEndTagName,
    ScriptDataLessThanSign,
    ScriptDataEndTagOpen,
    ScriptDataEndTagName,
    ScriptDataEscapeStart,
    ScriptDataEscapeStartDash,
    ScriptDataEscaped,
    ScriptDataEscapedDash,
    ScriptDataEscapedDashDash,
    ScriptDataEscapedLessThanSign,
    ScriptDataEscapedEndTagOpen,
    ScriptDataEscapedEndTagName,
    ScriptDataDoubleEscapeStart,
    ScriptDataDoubleEscaped,
    ScriptDataDoubleEscapedDash,
    ScriptDataDoubleEscapedDashDash,
    ScriptDataDoubleEscapedLessThanSign,
    ScriptDataDoubleEscapeEnd,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    BogusComment,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentLessThanSign,
    CommentLessThanSignBang,
    CommentLessThanSignBangDash,
    CommentLessThanSignBangDashDash,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    DOCTYPE,
    BeforeDOCTYPEName,
    DOCTYPEName,
    AfterDOCTYPEName,
    AfterDOCTYPEPublicKeyword,
    BeforeDOCTYPEPublicIdentifier,
    DOCTYPEPublicIdentifierDoubleQuoted,
    DOCTYPEPublicIdentifierSingleQuoted,
    AfterDOCTYPEPublicIdentifier,
    BetweenDOCTYPEPublicAndSystemIdentifiers,
    AfterDOCTYPESystemKeyword,
    BeforeDOCTYPESystemIdentifier,
    DOCTYPESystemIdentifierDoubleQuoted,
    DOCTYPESystemIdentifierSingleQuoted,
    AfterDOCTYPESystemIdentifier,
    BogusDOCTYPE,
    CDATASection,
    CDATASectionBracket,
    CDATASectionEnd,
    CharacterReference,
    NamedCharacterReference,
    AmbiguousAmpersand,
    NumericCharacterReference,
    HexadecimalCharacterReferenceStart,
    DecimalCharacterReferenceStart,
    HexadecimalCharacterReference,
    DecimalCharacterReference,
    NumericCharacterReferenceEnd,
}

pub struct HTMLTokenizer {
    state: TokenizerState,
    return_state: Option<TokenizerState>,
    input: String,
    current_pos: usize,
    current_input_character: Option<char>,
    current_token: Option<Token>,
    at_eof: bool,
    token_stream: Vec<Token>,
    // When true, the next iteration of the main loop will not consume a new character.
    // Spec: "Reconsume in the X state" sets this flag.
    reconsume: bool,
}
impl HTMLTokenizer {
    pub fn new(input: String) -> Self {
        // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
        // "The tokenizer state machine consists of the states defined in the
        // following subsections. The initial state is the data state."
        HTMLTokenizer {
            state: TokenizerState::Data,
            return_state: None,
            input,
            current_pos: 0,
            current_input_character: None,
            current_token: None,
            at_eof: false,
            token_stream: Vec::new(),
            reconsume: false,
        }
    }

    /// Consume the tokenizer and return the token stream.
    /// Call this after run() to get the tokens for the parser.
    pub fn into_tokens(self) -> Vec<Token> {
        self.token_stream
    }

    // Spec: "Switch to the X state"
    // Transitions to a new state. The next character will be consumed on the next
    // iteration of the main loop.
    fn switch_to(&mut self, new_state: TokenizerState) {
        self.state = new_state;
    }

    // Spec: "Reconsume in the X state"
    // Transitions to a new state without consuming the current character.
    // The same character will be processed again in the new state.
    fn reconsume_in(&mut self, new_state: TokenizerState) {
        self.reconsume = true;
        self.state = new_state;
    }

    fn log_parse_error(&self) {
        println!("Parse error at position {}", self.current_pos);
    }
    fn is_whitespace_char(input_char: char) -> bool {
        matches!(input_char, ' ' | '\t' | '\n' | '\x0C')
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "Emit the current token" - this adds the token to the output stream.
    pub fn emit_token(&mut self) {
        if let Some(token) = self.current_token.take() {
            println!("Token: {}", token);
            self.token_stream.push(token);
        }
    }

    // Emit a character token directly without going through current_token.
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#data-state
    // "Emit the current input character as a character token."
    pub fn emit_character_token(&mut self, c: char) {
        let token = Token::new_character(c);
        println!("Token: {}", token);
        self.token_stream.push(token);
    }

    // Emit an EOF token.
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tokenization
    // "Emit an end-of-file token."
    pub fn emit_eof_token(&mut self) {
        let token = Token::new_eof();
        println!("Token: {}", token);
        self.token_stream.push(token);
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#data-state
    fn handle_data_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0026 AMPERSAND (&) - Set the return state to the data state.
            // Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::Data);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // Spec: "U+003C LESS-THAN SIGN (<) - Switch to the tag open state."
            Some('<') => {
                self.switch_to(TokenizerState::TagOpen);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error.
            // Emit the current input character as a character token."
            Some('\0') => {
                self.log_parse_error();
                self.emit_character_token('\0');
                self.switch_to(TokenizerState::Data);
            }
            // Spec: "EOF - Emit an end-of-file token."
            None => {
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Emit the current input character as a character token."
            Some(c) => {
                self.emit_character_token(c);
                self.switch_to(TokenizerState::Data);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tag-open-state
    fn handle_tag_open_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0021 EXCLAMATION MARK (!) - Switch to the markup declaration open state."
            // NOTE: We use reconsume_in here so that MarkupDeclarationOpen can peek ahead
            // without the main loop consuming a character first. This state uses lookahead
            // rather than consuming the "current input character".
            Some('!') => {
                self.reconsume_in(TokenizerState::MarkupDeclarationOpen);
            }
            // Spec: "U+002F SOLIDUS (/) - Switch to the end tag open state."
            Some('/') => {
                self.switch_to(TokenizerState::EndTagOpen);
            }
            // Spec: "ASCII alpha - Create a new start tag token, set its tag name to the empty
            // string. Reconsume in the tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_start_tag());
                self.reconsume_in(TokenizerState::TagName);
            }
            // Spec: "U+003F QUESTION MARK (?) - This is an unexpected-question-mark-instead-of-tag-name
            // parse error. Create a comment token whose data is the empty string. Reconsume in the
            // bogus comment state."
            Some('?') => {
                self.log_parse_error();
                self.current_token = Some(Token::new_comment());
                self.reconsume_in(TokenizerState::BogusComment);
            }
            // Spec: "EOF - This is an eof-before-tag-name parse error. Emit a U+003C LESS-THAN SIGN
            // character token and an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - This is an invalid-first-character-of-tag-name parse error.
            // Emit a U+003C LESS-THAN SIGN character token. Reconsume in the data state."
            Some(_) => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.reconsume_in(TokenizerState::Data);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state
    fn handle_markup_declaration_open_state(&mut self) {
        // Spec: "If the next two characters are both U+002D HYPHEN-MINUS characters (-),
        // consume those two characters, create a comment token whose data is the empty
        // string, and switch to the comment start state."
        if self.next_few_characters_are("--") {
            self.consume_string("--");
            self.current_token = Some(Token::new_comment());
            self.switch_to(TokenizerState::CommentStart);
        }
        // Spec: "Otherwise, if the next seven characters are an ASCII case-insensitive
        // match for the word 'DOCTYPE', consume those characters and switch to the
        // DOCTYPE state."
        else if self.next_few_characters_are_case_insensitive("DOCTYPE") {
            self.consume_string("DOCTYPE");
            self.switch_to(TokenizerState::DOCTYPE);
        }
        // Spec: "Otherwise, if there is an adjusted current node and it is not an element
        // in the HTML namespace and the next seven characters are a case-sensitive match
        // for the string '[CDATA[', then consume those characters and switch to the
        // CDATA section state."
        else if self.next_few_characters_are("[CDATA[") {
            // TODO: Check adjusted current node condition
            self.consume_string("[CDATA[");
            self.switch_to(TokenizerState::CDATASection);
        }
        // Spec: "Otherwise, this is an incorrectly-opened-comment parse error. Create a
        // comment token whose data is the empty string. Switch to the bogus comment state
        // (don't consume anything in the current state)."
        else {
            self.log_parse_error();
            self.current_token = Some(Token::new_comment());
            self.reconsume_in(TokenizerState::BogusComment);
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#doctype-state
    fn handle_doctype_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before DOCTYPE name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeDOCTYPEName);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Reconsume in the before DOCTYPE name state."
            Some('>') => {
                self.reconsume_in(TokenizerState::BeforeDOCTYPEName);
            }
            // Spec: "EOF - This is an eof-in-doctype parse error. Create a new DOCTYPE token.
            // Set its force-quirks flag to on. Emit the current token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                let mut token = Token::new_doctype();
                token.set_force_quirks();
                self.current_token = Some(token);
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - This is a missing-whitespace-before-doctype-name parse error.
            // Reconsume in the before DOCTYPE name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeDOCTYPEName);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-name-state
    fn handle_before_doctype_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeDOCTYPEName);
            }
            // Spec: "ASCII upper alpha - Create a new DOCTYPE token. Set the token's name to
            // the lowercase version of the current input character. Switch to the DOCTYPE name state."
            Some(c) if c.is_ascii_uppercase() => {
                let mut token = Token::new_doctype();
                token.append_to_doctype_name(c.to_ascii_lowercase());
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Create a new
            // DOCTYPE token. Set the token's name to a U+FFFD REPLACEMENT CHARACTER. Switch to
            // the DOCTYPE name state."
            Some('\0') => {
                self.log_parse_error();
                let mut token = Token::new_doctype();
                token.append_to_doctype_name('\u{FFFD}');
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is a missing-doctype-name parse error.
            // Create a new DOCTYPE token. Set its force-quirks flag to on. Switch to the data state.
            // Emit the current token."
            Some('>') => {
                self.log_parse_error();
                let mut token = Token::new_doctype();
                token.set_force_quirks();
                self.current_token = Some(token);
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-doctype parse error. Create a new DOCTYPE token.
            // Set its force-quirks flag to on. Emit the current token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                let mut token = Token::new_doctype();
                token.set_force_quirks();
                self.current_token = Some(token);
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Create a new DOCTYPE token. Set the token's name to the
            // current input character. Switch to the DOCTYPE name state."
            Some(c) => {
                let mut token = Token::new_doctype();
                token.append_to_doctype_name(c);
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#doctype-name-state
    fn handle_doctype_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the after DOCTYPE name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::AfterDOCTYPEName);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current DOCTYPE token's name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name(c.to_ascii_lowercase());
                }
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current DOCTYPE token's name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name('\u{FFFD}');
                }
            }
            // Spec: "EOF - This is an eof-in-doctype parse error. Set the current DOCTYPE token's
            // force-quirks flag to on. Emit the current token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.set_force_quirks();
                }
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the current DOCTYPE
            // token's name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name(c);
                }
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state
    fn handle_tag_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // Spec: "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current tag token's tag name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c.to_ascii_lowercase());
                }
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current tag token's tag name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name('\u{FFFD}');
                }
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the current tag
            // token's tag name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c);
                }
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#self-closing-start-tag-state
    fn handle_self_closing_start_tag_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+003E GREATER-THAN SIGN (>) - Set the self-closing flag of the current
            // tag token. Switch to the data state. Emit the current token."
            Some('>') => {
                if let Some(ref mut token) = self.current_token {
                    token.set_self_closing();
                }
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - This is an unexpected-solidus-in-tag parse error.
            // Reconsume in the before attribute name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeAttributeName);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#end-tag-open-state
    fn handle_end_tag_open_state(&mut self) {
        match self.current_input_character {
            // Spec: "ASCII alpha - Create a new end tag token, set its tag name to the empty
            // string. Reconsume in the tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_end_tag());
                self.reconsume_in(TokenizerState::TagName);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is a missing-end-tag-name parse error.
            // Switch to the data state."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
            }
            // Spec: "EOF - This is an eof-before-tag-name parse error. Emit a U+003C LESS-THAN
            // SIGN character token, a U+002F SOLIDUS character token and an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.emit_character_token('/');
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - This is an invalid-first-character-of-tag-name parse error.
            // Create a comment token whose data is the empty string. Reconsume in the bogus
            // comment state."
            Some(_) => {
                self.log_parse_error();
                self.current_token = Some(Token::new_comment());
                self.reconsume_in(TokenizerState::BogusComment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state
    fn handle_before_attribute_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // Spec: "U+002F SOLIDUS (/), U+003E GREATER-THAN SIGN (>), EOF -
            // Reconsume in the after attribute name state."
            Some('/') | Some('>') | None => {
                self.reconsume_in(TokenizerState::AfterAttributeName);
            }
            // Spec: "U+003D EQUALS SIGN (=) - This is an unexpected-equals-sign-before-attribute-name
            // parse error. Start a new attribute in the current tag token. Set that attribute's name
            // to the current input character, and its value to the empty string. Switch to the
            // attribute name state."
            Some('=') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.start_new_attribute();
                    token.append_to_current_attribute_name('=');
                }
                self.switch_to(TokenizerState::AttributeName);
            }
            // Spec: "Anything else - Start a new attribute in the current tag token. Set that
            // attribute name and value to the empty string. Reconsume in the attribute name state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.start_new_attribute();
                }
                self.reconsume_in(TokenizerState::AttributeName);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state
    fn handle_attribute_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE, U+002F SOLIDUS (/), U+003E GREATER-THAN SIGN (>), EOF -
            // Reconsume in the after attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.check_duplicate_attribute();
                self.reconsume_in(TokenizerState::AfterAttributeName);
            }
            Some('/') | Some('>') => {
                self.check_duplicate_attribute();
                self.reconsume_in(TokenizerState::AfterAttributeName);
            }
            None => {
                self.check_duplicate_attribute();
                self.reconsume_in(TokenizerState::AfterAttributeName);
            }
            // Spec: "U+003D EQUALS SIGN (=) - Switch to the before attribute value state."
            Some('=') => {
                self.check_duplicate_attribute();
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // Spec: "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current attribute's name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(c.to_ascii_lowercase());
                }
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name('\u{FFFD}');
                }
            }
            // Spec: "U+0022 QUOTATION MARK (\"), U+0027 APOSTROPHE ('), U+003C LESS-THAN SIGN (<) -
            // This is an unexpected-character-in-attribute-name parse error. Treat it as per the
            // 'anything else' entry below."
            Some('"') | Some('\'') | Some('<') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(self.current_input_character.unwrap());
                }
            }
            // Spec: "Anything else - Append the current input character to the current attribute's name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(c);
                }
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-name-state
    fn handle_after_attribute_name_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::AfterAttributeName);
            }
            // Spec: "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // Spec: "U+003D EQUALS SIGN (=) - Switch to the before attribute value state."
            Some('=') => {
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Start a new attribute in the current tag token. Set that
            // attribute name and value to the empty string. Reconsume in the attribute name state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.start_new_attribute();
                }
                self.reconsume_in(TokenizerState::AttributeName);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-value-state
    fn handle_before_attribute_value_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // Spec: "U+0022 QUOTATION MARK (\") - Switch to the attribute value (double-quoted) state."
            Some('"') => {
                self.switch_to(TokenizerState::AttributeValueDoubleQuoted);
            }
            // Spec: "U+0027 APOSTROPHE (') - Switch to the attribute value (single-quoted) state."
            Some('\'') => {
                self.switch_to(TokenizerState::AttributeValueSingleQuoted);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is a missing-attribute-value parse error.
            // Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "Anything else - Reconsume in the attribute value (unquoted) state."
            _ => {
                self.reconsume_in(TokenizerState::AttributeValueUnquoted);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state
    fn handle_attribute_value_double_quoted_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0022 QUOTATION MARK (\") - Switch to the after attribute value (quoted) state."
            Some('"') => {
                self.switch_to(TokenizerState::AfterAttributeValueQuoted);
            }
            // Spec: "U+0026 AMPERSAND (&) - Set the return state to the attribute value (double-quoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueDoubleQuoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(single-quoted)-state
    fn handle_attribute_value_single_quoted_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0027 APOSTROPHE (') - Switch to the after attribute value (quoted) state."
            Some('\'') => {
                self.switch_to(TokenizerState::AfterAttributeValueQuoted);
            }
            // Spec: "U+0026 AMPERSAND (&) - Set the return state to the attribute value (single-quoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueSingleQuoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(unquoted)-state
    fn handle_attribute_value_unquoted_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // Spec: "U+0026 AMPERSAND (&) - Set the return state to the attribute value (unquoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueUnquoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // Spec: "U+0022 QUOTATION MARK (\"), U+0027 APOSTROPHE ('), U+003C LESS-THAN SIGN (<),
            // U+003D EQUALS SIGN (=), U+0060 GRAVE ACCENT (`) - This is an
            // unexpected-character-in-unquoted-attribute-value parse error. Treat it as per the
            // 'anything else' entry below."
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(self.current_input_character.unwrap());
                }
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-value-(quoted)-state
    fn handle_after_attribute_value_quoted_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // Spec: "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - This is a missing-whitespace-between-attributes parse error.
            // Reconsume in the before attribute name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeAttributeName);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Comment states
    // -------------------------------------------------------------------------

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-start-state
    fn handle_comment_start_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment start dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentStartDash);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is an abrupt-closing-of-empty-comment
            // parse error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-start-dash-state
    fn handle_comment_start_dash_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment end state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEnd);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is an abrupt-closing-of-empty-comment
            // parse error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append a U+002D HYPHEN-MINUS character (-) to the comment
            // token's data. Reconsume in the comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-state
    fn handle_comment_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+003C LESS-THAN SIGN (<) - Append the current input character to the
            // comment token's data. Switch to the comment less-than sign state."
            Some('<') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('<');
                }
                self.switch_to(TokenizerState::CommentLessThanSign);
            }
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment end dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEndDash);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER character to the comment token's data."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('\u{FFFD}');
                }
            }
            // Spec: "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append the current input character to the comment token's data."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment(c);
                }
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-state
    fn handle_comment_less_than_sign_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+0021 EXCLAMATION MARK (!) - Append the current input character to the
            // comment token's data. Switch to the comment less-than sign bang state."
            Some('!') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('!');
                }
                self.switch_to(TokenizerState::CommentLessThanSignBang);
            }
            // Spec: "U+003C LESS-THAN SIGN (<) - Append the current input character to the
            // comment token's data."
            Some('<') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('<');
                }
            }
            // Spec: "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-state
    fn handle_comment_less_than_sign_bang_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment less-than sign bang dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentLessThanSignBangDash);
            }
            // Spec: "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-state
    fn handle_comment_less_than_sign_bang_dash_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment less-than sign bang dash dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentLessThanSignBangDashDash);
            }
            // Spec: "Anything else - Reconsume in the comment end dash state."
            _ => {
                self.reconsume_in(TokenizerState::CommentEndDash);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-dash-state
    fn handle_comment_less_than_sign_bang_dash_dash_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+003E GREATER-THAN SIGN (>) - Reconsume in the comment end state."
            Some('>') => {
                self.reconsume_in(TokenizerState::CommentEnd);
            }
            // Spec: "EOF - Reconsume in the comment end state."
            None => {
                self.reconsume_in(TokenizerState::CommentEnd);
            }
            // Spec: "Anything else - This is a nested-comment parse error. Reconsume in the
            // comment end state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::CommentEnd);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-end-dash-state
    fn handle_comment_end_dash_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Switch to the comment end state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEnd);
            }
            // Spec: "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append a U+002D HYPHEN-MINUS character (-) to the comment
            // token's data. Reconsume in the comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-end-state
    fn handle_comment_end_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current
            // comment token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "U+0021 EXCLAMATION MARK (!) - Switch to the comment end bang state."
            Some('!') => {
                self.switch_to(TokenizerState::CommentEndBang);
            }
            // Spec: "U+002D HYPHEN-MINUS (-) - Append a U+002D HYPHEN-MINUS character (-) to
            // the comment token's data."
            Some('-') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
            }
            // Spec: "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append two U+002D HYPHEN-MINUS characters (-) to the
            // comment token's data. Reconsume in the comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                    token.append_to_comment('-');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#comment-end-bang-state
    fn handle_comment_end_bang_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+002D HYPHEN-MINUS (-) - Append two U+002D HYPHEN-MINUS characters (-)
            // and a U+0021 EXCLAMATION MARK character (!) to the comment token's data. Switch
            // to the comment end dash state."
            Some('-') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                    token.append_to_comment('-');
                    token.append_to_comment('!');
                }
                self.switch_to(TokenizerState::CommentEndDash);
            }
            // Spec: "U+003E GREATER-THAN SIGN (>) - This is an incorrectly-closed-comment parse
            // error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "Anything else - Append two U+002D HYPHEN-MINUS characters (-) and a U+0021
            // EXCLAMATION MARK character (!) to the comment token's data. Reconsume in the
            // comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                    token.append_to_comment('-');
                    token.append_to_comment('!');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#bogus-comment-state
    fn handle_bogus_comment_state(&mut self) {
        match self.current_input_character {
            // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current
            // comment token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // Spec: "EOF - Emit the comment. Emit an end-of-file token."
            None => {
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER character to the comment token's data."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('\u{FFFD}');
                }
            }
            // Spec: "Anything else - Append the current input character to the comment token's data."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment(c);
                }
            }
        }
    }

    /// Helper to check for duplicate attributes and handle the parse error.
    fn check_duplicate_attribute(&mut self) {
        // Check for duplicate first, then log error and remove if needed.
        // This avoids borrow checker issues by not holding a mutable borrow
        // while calling log_parse_error.
        let is_duplicate = self
            .current_token
            .as_ref()
            .map(|t| t.current_attribute_name_is_duplicate())
            .unwrap_or(false);

        if is_duplicate {
            self.log_parse_error();
            if let Some(ref mut token) = self.current_token {
                token.remove_current_attribute();
            }
        }
    }

    // Spec: "Consume the next input character"
    // Returns the character at the current position and advances the position.
    fn consume(&mut self) -> Option<char> {
        if let Some(c) = self.input[self.current_pos..].chars().next() {
            self.current_pos += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    /// Check if the next few characters match the target string exactly.
    pub fn next_few_characters_are(&self, target: &str) -> bool {
        let target_chars: Vec<char> = target.chars().collect();

        for (i, target_char) in target_chars.iter().enumerate() {
            match self.peek_codepoint(i) {
                Some(input_char) => {
                    if input_char != *target_char {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }

    /// Check if the next few characters match the target string (ASCII case-insensitive).
    /// Spec: https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state
    /// "ASCII case-insensitive match"
    pub fn next_few_characters_are_case_insensitive(&self, target: &str) -> bool {
        let target_chars: Vec<char> = target.chars().collect();

        for (i, target_char) in target_chars.iter().enumerate() {
            match self.peek_codepoint(i) {
                Some(input_char) => {
                    if !input_char.eq_ignore_ascii_case(target_char) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
    /// Consume the given string from the input.
    /// Caller must have already verified the characters are present.
    pub fn consume_string(&mut self, target: &str) {
        // Advance by the number of bytes in the target string.
        // This is safe for ASCII strings (like "DOCTYPE", "--", "[CDATA[").
        self.current_pos += target.len();
    }
    // Use peek to view the next codepoint at a given offset without advancing
    pub fn peek_codepoint(&self, offset: usize) -> Option<char> {
        let slice = &self.input[self.current_pos..]; // Slice from the current position
                                                     // The slice should always start from where we are in the string
                                                     // println!("Slice to peek: {}", slice);

        slice.chars().nth(offset) // Get the character at the `offset` in the current slice
    }

    pub fn run(&mut self) {
        loop {
            // Spec: Each state begins by consuming the next input character,
            // unless we're reconsuming from a previous state transition.
            if self.reconsume {
                self.reconsume = false;
                // Keep current_input_character as-is for reconsuming
            } else {
                self.current_input_character = self.consume();
            }

            if self.current_input_character.is_none() && self.at_eof {
                println!();
                break;
            }
            match self.state {
                TokenizerState::Data => {
                    self.handle_data_state();
                    continue;
                }
                TokenizerState::RCDATA => todo!("Unhandled state: {}", self.state),
                TokenizerState::RAWTEXT => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptData => todo!("Unhandled state: {}", self.state),
                TokenizerState::PLAINTEXT => todo!("Unhandled state: {}", self.state),
                TokenizerState::TagOpen => {
                    self.handle_tag_open_state();
                    continue;
                }
                TokenizerState::EndTagOpen => {
                    self.handle_end_tag_open_state();
                    continue;
                }
                TokenizerState::TagName => {
                    self.handle_tag_name_state();
                    continue;
                }
                TokenizerState::RCDATALessThanSign => todo!("Unhandled state: {}", self.state),
                TokenizerState::RCDATAEndTagOpen => todo!("Unhandled state: {}", self.state),
                TokenizerState::RCDATAEndTagName => todo!("Unhandled state: {}", self.state),
                TokenizerState::RAWTEXTLessThanSign => todo!("Unhandled state: {}", self.state),
                TokenizerState::RAWTEXTEndTagOpen => todo!("Unhandled state: {}", self.state),
                TokenizerState::RAWTEXTEndTagName => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataLessThanSign => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEndTagOpen => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEndTagName => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEscapeStart => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEscapeStartDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataEscaped => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEscapedDash => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataEscapedDashDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataEscapedLessThanSign => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataEscapedEndTagOpen => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataEscapedEndTagName => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataDoubleEscapeStart => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataDoubleEscaped => todo!("Unhandled state: {}", self.state),
                TokenizerState::ScriptDataDoubleEscapedDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataDoubleEscapedDashDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataDoubleEscapedLessThanSign => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::ScriptDataDoubleEscapeEnd => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::BeforeAttributeName => {
                    self.handle_before_attribute_name_state();
                    continue;
                }
                TokenizerState::AttributeName => {
                    self.handle_attribute_name_state();
                    continue;
                }
                TokenizerState::AfterAttributeName => {
                    self.handle_after_attribute_name_state();
                    continue;
                }
                TokenizerState::BeforeAttributeValue => {
                    self.handle_before_attribute_value_state();
                    continue;
                }
                TokenizerState::AttributeValueDoubleQuoted => {
                    self.handle_attribute_value_double_quoted_state();
                    continue;
                }
                TokenizerState::AttributeValueSingleQuoted => {
                    self.handle_attribute_value_single_quoted_state();
                    continue;
                }
                TokenizerState::AttributeValueUnquoted => {
                    self.handle_attribute_value_unquoted_state();
                    continue;
                }
                TokenizerState::AfterAttributeValueQuoted => {
                    self.handle_after_attribute_value_quoted_state();
                    continue;
                }
                TokenizerState::SelfClosingStartTag => {
                    self.handle_self_closing_start_tag_state();
                    continue;
                }
                TokenizerState::BogusComment => {
                    self.handle_bogus_comment_state();
                    continue;
                }
                TokenizerState::MarkupDeclarationOpen => {
                    self.handle_markup_declaration_open_state();
                    continue;
                }
                TokenizerState::CommentStart => {
                    self.handle_comment_start_state();
                    continue;
                }
                TokenizerState::CommentStartDash => {
                    self.handle_comment_start_dash_state();
                    continue;
                }
                TokenizerState::Comment => {
                    self.handle_comment_state();
                    continue;
                }
                TokenizerState::CommentLessThanSign => {
                    self.handle_comment_less_than_sign_state();
                    continue;
                }
                TokenizerState::CommentLessThanSignBang => {
                    self.handle_comment_less_than_sign_bang_state();
                    continue;
                }
                TokenizerState::CommentLessThanSignBangDash => {
                    self.handle_comment_less_than_sign_bang_dash_state();
                    continue;
                }
                TokenizerState::CommentLessThanSignBangDashDash => {
                    self.handle_comment_less_than_sign_bang_dash_dash_state();
                    continue;
                }
                TokenizerState::CommentEndDash => {
                    self.handle_comment_end_dash_state();
                    continue;
                }
                TokenizerState::CommentEnd => {
                    self.handle_comment_end_state();
                    continue;
                }
                TokenizerState::CommentEndBang => {
                    self.handle_comment_end_bang_state();
                    continue;
                }
                TokenizerState::DOCTYPE => {
                    self.handle_doctype_state();
                    continue;
                }
                TokenizerState::BeforeDOCTYPEName => {
                    self.handle_before_doctype_name_state();
                    continue;
                }
                TokenizerState::DOCTYPEName => {
                    self.handle_doctype_name_state();
                    continue;
                }
                TokenizerState::AfterDOCTYPEName => todo!("Unhandled state: {}", self.state),
                TokenizerState::AfterDOCTYPEPublicKeyword => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::BeforeDOCTYPEPublicIdentifier => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DOCTYPEPublicIdentifierDoubleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DOCTYPEPublicIdentifierSingleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::AfterDOCTYPEPublicIdentifier => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::BetweenDOCTYPEPublicAndSystemIdentifiers => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::AfterDOCTYPESystemKeyword => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::BeforeDOCTYPESystemIdentifier => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DOCTYPESystemIdentifierDoubleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DOCTYPESystemIdentifierSingleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::AfterDOCTYPESystemIdentifier => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::BogusDOCTYPE => todo!("Unhandled state: {}", self.state),
                TokenizerState::CDATASection => todo!("Unhandled state: {}", self.state),
                TokenizerState::CDATASectionBracket => todo!("Unhandled state: {}", self.state),
                TokenizerState::CDATASectionEnd => todo!("Unhandled state: {}", self.state),
                TokenizerState::CharacterReference => todo!("Unhandled state: {}", self.state),
                TokenizerState::NamedCharacterReference => todo!("Unhandled state: {}", self.state),
                TokenizerState::AmbiguousAmpersand => todo!("Unhandled state: {}", self.state),
                TokenizerState::NumericCharacterReference => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::HexadecimalCharacterReferenceStart => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DecimalCharacterReferenceStart => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::HexadecimalCharacterReference => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::DecimalCharacterReference => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::NumericCharacterReferenceEnd => {
                    todo!("Unhandled state: {}", self.state)
                }
            }
        }
    }
}
