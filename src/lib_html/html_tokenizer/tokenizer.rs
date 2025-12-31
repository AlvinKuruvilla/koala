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
        }
    }

    // Transition to a new state
    fn switch_to(&mut self, new_state: TokenizerState) {
        // println!("Switched from: {} to {}", self.state, new_state);
        self.state = new_state;
        self.current_input_character = self.next_codepoint(false);
    }
    fn switch_to_without_consume(&mut self, new_state: TokenizerState) {
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
            Some('!') => {
                self.switch_to(TokenizerState::MarkupDeclarationOpen);
            }
            // Spec: "U+002F SOLIDUS (/) - Switch to the end tag open state."
            Some('/') => {
                self.switch_to(TokenizerState::EndTagOpen);
            }
            // Spec: "ASCII alpha - Create a new start tag token, set its tag name to the empty
            // string. Reconsume in the tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_start_tag());
                self.switch_to_without_consume(TokenizerState::TagName);
            }
            // Spec: "U+003F QUESTION MARK (?) - This is an unexpected-question-mark-instead-of-tag-name
            // parse error. Create a comment token whose data is the empty string. Reconsume in the
            // bogus comment state."
            Some('?') => {
                self.log_parse_error();
                self.current_token = Some(Token::new_comment());
                self.switch_to_without_consume(TokenizerState::BogusComment);
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
                self.switch_to_without_consume(TokenizerState::Data);
            }
        }
    }
    // Spec: https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state
    fn handle_markup_declaration_open_state(&mut self) {
        // Spec: "If the next two characters are both U+002D HYPHEN-MINUS characters (-),
        // consume those two characters, create a comment token whose data is the empty
        // string, and switch to the comment start state."
        if self.next_few_characters_are("--") {
            self.consume("--");
            self.current_token = Some(Token::new_comment());
            self.switch_to_without_consume(TokenizerState::CommentStart);
        }
        // Spec: "Otherwise, if the next seven characters are an ASCII case-insensitive
        // match for the word 'DOCTYPE', consume those characters and switch to the
        // DOCTYPE state."
        else if self.next_few_characters_are_case_insensitive("DOCTYPE") {
            self.consume("DOCTYPE");
            self.switch_to_without_consume(TokenizerState::DOCTYPE);
        }
        // Spec: "Otherwise, if there is an adjusted current node and it is not an element
        // in the HTML namespace and the next seven characters are a case-sensitive match
        // for the string '[CDATA[', then consume those characters and switch to the
        // CDATA section state."
        else if self.next_few_characters_are("[CDATA[") {
            // TODO: Check adjusted current node condition
            self.consume("[CDATA[");
            self.switch_to_without_consume(TokenizerState::CDATASection);
        }
        // Spec: "Otherwise, this is an incorrectly-opened-comment parse error. Create a
        // comment token whose data is the empty string. Switch to the bogus comment state
        // (don't consume anything in the current state)."
        else {
            self.log_parse_error();
            self.current_token = Some(Token::new_comment());
            self.switch_to_without_consume(TokenizerState::BogusComment);
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
                self.switch_to_without_consume(TokenizerState::BeforeDOCTYPEName);
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
                self.switch_to_without_consume(TokenizerState::BeforeDOCTYPEName);
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
                self.current_input_character = self.next_codepoint(false);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current DOCTYPE token's name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name('\u{FFFD}');
                }
                self.current_input_character = self.next_codepoint(false);
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
                self.current_input_character = self.next_codepoint(false);
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
                self.current_input_character = self.next_codepoint(false);
            }
            // Spec: "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current tag token's tag name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name('\u{FFFD}');
                }
                self.current_input_character = self.next_codepoint(false);
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
                self.current_input_character = self.next_codepoint(false);
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
                self.switch_to_without_consume(TokenizerState::BeforeAttributeName);
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
                self.switch_to_without_consume(TokenizerState::TagName);
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
                self.switch_to_without_consume(TokenizerState::BogusComment);
            }
        }
    }
    /// Retrieve the next code point (character) and update the position
    pub fn next_codepoint(&mut self, is_parsing_first_char: bool) -> Option<char> {
        // Print the current position and the remaining string
        // println!("Current position: {}", self.current_pos);
        // println!(
        //     "In next_codepoint: Remaining string: {}",
        //     &self.input[self.current_pos..]
        // );

        // Get the next character at the current position
        if let Some(code_point) = self.input[self.current_pos..].chars().next() {
            // println!("In next_codepoint: Current char: {:?}", code_point); // Print the current character

            // Update the position by advancing past the current code point
            if !is_parsing_first_char {
                self.current_pos += code_point.len_utf8();
            }
            // println!(
            //     "In next_codepoint: New position after consuming: {}",
            //     self.current_pos
            // );

            Some(code_point)
        } else {
            None // Return None if we've reached the end of the string
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
    pub fn consume(&mut self, target: &str) {
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
            self.current_input_character = self.next_codepoint(true);
            // println!(
            //     "Current char: {:?} at position: {}",
            //     self.current_input_character, self.current_pos
            // );
            // println!(
            //     "self.current_input_character.is_none: {}",
            //     self.current_input_character.is_none()
            // );
            // println!(
            //     "self.current_token.is_eof(): {} because current token is: {}",
            //     self.current_token.is_eof(),
            //     self.current_token
            // );
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
                TokenizerState::BeforeAttributeName => todo!("Unhandled state: {}", self.state),
                TokenizerState::AttributeName => todo!("Unhandled state: {}", self.state),
                TokenizerState::AfterAttributeName => todo!("Unhandled state: {}", self.state),
                TokenizerState::BeforeAttributeValue => todo!("Unhandled state: {}", self.state),
                TokenizerState::AttributeValueDoubleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::AttributeValueSingleQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::AttributeValueUnquoted => todo!("Unhandled state: {}", self.state),
                TokenizerState::AfterAttributeValueQuoted => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::SelfClosingStartTag => {
                    self.handle_self_closing_start_tag_state();
                    continue;
                }
                TokenizerState::BogusComment => todo!("Unhandled state: {}", self.state),
                TokenizerState::MarkupDeclarationOpen => {
                    self.handle_markup_declaration_open_state();
                    continue;
                }
                TokenizerState::CommentStart => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentStartDash => todo!("Unhandled state: {}", self.state),
                TokenizerState::Comment => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentLessThanSign => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentLessThanSignBang => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentLessThanSignBangDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::CommentLessThanSignBangDashDash => {
                    todo!("Unhandled state: {}", self.state)
                }
                TokenizerState::CommentEndDash => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentEnd => todo!("Unhandled state: {}", self.state),
                TokenizerState::CommentEndBang => todo!("Unhandled state: {}", self.state),
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
