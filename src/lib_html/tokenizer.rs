use std::io::{self, Read, Write};
use strum_macros::Display;

use super::token::{HTMLToken, HTMLTokenType};
fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}

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
    current_token: HTMLToken,
    at_eof: bool,
}
impl HTMLTokenizer {
    pub fn new(input: String) -> Self {
        // Initialize with the `Data` state
        HTMLTokenizer {
            state: TokenizerState::Data,
            return_state: Some(TokenizerState::Data),
            input,
            current_pos: 0,
            current_input_character: None,
            current_token: HTMLToken::new(super::token::HTMLTokenType::DOCTYPE),
            at_eof: false,
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
    pub fn emit_token(&mut self) {
        if self.current_token.is_eof() || (!self.current_token.doctype_data().name().is_empty()) {
            println!("Token: {}", self.current_token);
        }
        self.current_token = HTMLToken::new(super::token::HTMLTokenType::DOCTYPE);
    }
    fn handle_data_state(&mut self) {
        match self.current_input_character {
            Some('&') => {
                self.return_state = Some(TokenizerState::Data);
                self.switch_to(TokenizerState::CharacterReference);
            }
            Some('<') => {
                self.switch_to(TokenizerState::TagOpen);
            }
            Some('\n') => {
                if self.current_token.token_type() != HTMLTokenType::Character {
                    self.current_token
                        .set_token_type(super::token::HTMLTokenType::Character);
                }
                self.current_token
                    .doctype_data()
                    .append_character_to_name(self.current_input_character.unwrap());
                self.emit_token();
                self.switch_to(TokenizerState::Data);
            }
            None => {
                // println!("Emitting End of file token");
                self.current_token
                    .set_token_type(super::token::HTMLTokenType::EndOfFile);
                self.emit_token();
                self.at_eof = true;
            }
            _ => todo!(
                " Unhandled input character: {:?}",
                self.current_input_character
            ),
        }
    }
    // SPEC: https://html.spec.whatwg.org/multipage/parsing.html#tag-open-state
    fn handle_tag_open_state(&mut self) {
        match self.current_input_character {
            Some(_) => {
                if self.current_input_character == Some('!') {
                    // println!("HERE char is {}", self.current_input_character.unwrap());
                    self.switch_to(TokenizerState::MarkupDeclarationOpen);
                    return;
                }
                if self.current_input_character.unwrap().is_ascii_alphabetic() {
                    self.current_token
                        .set_token_type(super::token::HTMLTokenType::StartTag);
                    // TODO: Do we to use self.switch_to() instead here?
                    self.switch_to_without_consume(TokenizerState::TagName);
                } else if self.current_input_character == Some('/') {
                    self.switch_to(TokenizerState::EndTagOpen);
                    return;
                } else {
                    todo!("Unhandled character: {:?}", self.current_input_character)
                }
            }
            None => todo!(),

            _ => {
                // TODO: This is a little unfortunate, but we will have to special case ampersand and null and any other specific cases in the spec
                //       before this becomes a true "ANYTHING ELSE" case
                self.emit_token();
                self.switch_to(TokenizerState::TagOpen);
            }
        }
    }
    fn handle_markup_decleration_open_state(&mut self) {
        // println!("in markup function");
        // println!("Current char: {:?}", self.current_input_character);
        match self.current_input_character {
            Some(_) => {
                if self.next_few_characters_are("DOCTYPE") {
                    self.consume("DOCTYPE");
                    // eprintln!("!!!!!!HERE");
                    // println!("Current char: {:?}", self.current_input_character);
                    self.switch_to_without_consume(TokenizerState::DOCTYPE);
                } else {
                    todo!(
                        "Unhandled input character: {:?}",
                        self.current_input_character
                    )
                }
            }
            None => todo!(),
        }
    }
    fn handle_doctype_tag(&mut self) {
        match self.current_input_character {
            Some(_) => {
                // println!(
                //     "In doctype handler current input char is: {}",
                //     self.current_input_character.unwrap()
                // );
                if Self::is_whitespace_char(self.current_input_character.unwrap()) {
                    self.switch_to(TokenizerState::BeforeDOCTYPEName);
                } else {
                    todo!()
                }
            }
            None => todo!(),
        }
    }
    fn handle_before_doctype_name_state(&mut self) {
        // println!(
        //     "Character in before doctype name state is: {}",
        //     self.current_input_character.unwrap()
        // );
        self.current_token
            .set_token_type(super::token::HTMLTokenType::DOCTYPE);
        self.current_token
            .doctype_data()
            .append_character_to_name(self.current_input_character.unwrap());
        // println!(
        //     "Current token name: {}",
        //     self.current_token.doctype_data().name()
        // );
        self.switch_to(TokenizerState::DOCTYPEName);
    }
    fn handle_doctype_name_state(&mut self) {
        match self.current_input_character {
            Some(c) => {
                if c == '>' {
                    self.switch_to(TokenizerState::Data);
                    self.emit_token();
                } else {
                    self.current_token
                        .doctype_data()
                        .append_character_to_name(self.current_input_character.unwrap());
                    // println!(
                    //     "Current token name: {}",
                    //     self.current_token.doctype_data().name()
                    // );
                    self.current_input_character = self.next_codepoint(false);
                }
            }
            None => todo!(),
        }
    }
    // SPEC: https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state
    fn handle_tag_name_state(&mut self) {
        match self.current_input_character {
            Some(c) => {
                if c == ' ' {
                    todo!()
                } else if c == '/' {
                    self.switch_to(TokenizerState::SelfClosingStartTag);
                } else if c == '>' {
                    self.emit_token();
                    self.switch_to(TokenizerState::Data);
                } else {
                    self.current_token
                        .doctype_data()
                        .append_character_to_name(self.current_input_character.unwrap());
                    // println!(
                    //     "Current token name: {}",
                    //     self.current_token.doctype_data().name()
                    // );
                    self.current_input_character = self.next_codepoint(false);
                }
            }
            None => todo!(),
        }
    }
    fn handle_self_closing_start_tag_state(&mut self) {
        match self.current_input_character {
            Some('>') => {
                self.current_token
                    .start_or_end_tag_data()
                    .set_self_closing_flag(true);
                self.emit_token();
                self.switch_to(TokenizerState::Data);
            }

            None => todo!(),
            _ => todo!(
                " Unhandled input character: {:?}",
                self.current_input_character
            ),
        }
    }
    fn handle_end_tag_open_state(&mut self) {
        match self.current_input_character {
            Some(_) => {
                if self.current_input_character.unwrap().is_ascii_alphabetic() {
                    self.current_token
                        .set_token_type(super::token::HTMLTokenType::EndTag);
                    // TODO: Do we to use self.switch_to() instead here?
                    self.switch_to_without_consume(TokenizerState::TagName);
                }
            }
            None => todo!(),
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
    /// Peek the next codepoint (character) from the input string at the given offset from the current position without advancing the position
    pub fn next_few_characters_are(&self, target: &str) -> bool {
        let target_chars: Vec<char> = target.chars().collect();

        // Peek ahead for each character in the target string
        for (i, target_char) in target_chars.iter().enumerate() {
            match self.peek_codepoint(i) {
                Some(input_char) => {
                    if input_char != *target_char {
                        return false; // Return false if any character doesn't match
                    }
                }
                None => return false, // Return false if out of bounds
            }
        }
        true // Return true if all characters match
    }
    pub fn consume(&mut self, target: &str) {
        // Assert the target matches the current input
        assert!(self.next_few_characters_are(target));
        self.current_pos += target.len(); // Move the position forward after confirming match
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
                    self.handle_markup_decleration_open_state();
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
                    self.handle_doctype_tag();
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
