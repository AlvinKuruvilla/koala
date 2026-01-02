use strum_macros::Display;

use super::token::Token;

/// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
///
/// The tokenizer state machine. Each state corresponds to a section in § 13.2.5.
#[derive(Debug, PartialEq, Display)]
pub enum TokenizerState {
    /// [§ 13.2.5.1 Data state](https://html.spec.whatwg.org/multipage/parsing.html#data-state)
    Data,
    /// [§ 13.2.5.2 RCDATA state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-state)
    RCDATA,
    /// [§ 13.2.5.3 RAWTEXT state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-state)
    RAWTEXT,
    /// [§ 13.2.5.4 Script data state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-state)
    ScriptData,
    /// [§ 13.2.5.5 PLAINTEXT state](https://html.spec.whatwg.org/multipage/parsing.html#plaintext-state)
    PLAINTEXT,
    /// [§ 13.2.5.6 Tag open state](https://html.spec.whatwg.org/multipage/parsing.html#tag-open-state)
    TagOpen,
    /// [§ 13.2.5.7 End tag open state](https://html.spec.whatwg.org/multipage/parsing.html#end-tag-open-state)
    EndTagOpen,
    /// [§ 13.2.5.8 Tag name state](https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state)
    TagName,
    /// [§ 13.2.5.9 RCDATA less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-less-than-sign-state)
    RCDATALessThanSign,
    /// [§ 13.2.5.10 RCDATA end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-open-state)
    RCDATAEndTagOpen,
    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    RCDATAEndTagName,
    /// [§ 13.2.5.12 RAWTEXT less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-less-than-sign-state)
    RAWTEXTLessThanSign,
    /// [§ 13.2.5.13 RAWTEXT end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-open-state)
    RAWTEXTEndTagOpen,
    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    RAWTEXTEndTagName,
    /// [§ 13.2.5.15 Script data less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-less-than-sign-state)
    ScriptDataLessThanSign,
    /// [§ 13.2.5.16 Script data end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-open-state)
    ScriptDataEndTagOpen,
    /// [§ 13.2.5.17 Script data end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-name-state)
    ScriptDataEndTagName,
    /// [§ 13.2.5.18 Script data escape start state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escape-start-state)
    ScriptDataEscapeStart,
    /// [§ 13.2.5.19 Script data escape start dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escape-start-dash-state)
    ScriptDataEscapeStartDash,
    /// [§ 13.2.5.20 Script data escaped state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-state)
    ScriptDataEscaped,
    /// [§ 13.2.5.21 Script data escaped dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-dash-state)
    ScriptDataEscapedDash,
    /// [§ 13.2.5.22 Script data escaped dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-dash-dash-state)
    ScriptDataEscapedDashDash,
    /// [§ 13.2.5.23 Script data escaped less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-less-than-sign-state)
    ScriptDataEscapedLessThanSign,
    /// [§ 13.2.5.24 Script data escaped end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-end-tag-open-state)
    ScriptDataEscapedEndTagOpen,
    /// [§ 13.2.5.25 Script data escaped end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-end-tag-name-state)
    ScriptDataEscapedEndTagName,
    /// [§ 13.2.5.26 Script data double escape start state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escape-start-state)
    ScriptDataDoubleEscapeStart,
    /// [§ 13.2.5.27 Script data double escaped state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-state)
    ScriptDataDoubleEscaped,
    /// [§ 13.2.5.28 Script data double escaped dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-dash-state)
    ScriptDataDoubleEscapedDash,
    /// [§ 13.2.5.29 Script data double escaped dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-dash-dash-state)
    ScriptDataDoubleEscapedDashDash,
    /// [§ 13.2.5.30 Script data double escaped less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-less-than-sign-state)
    ScriptDataDoubleEscapedLessThanSign,
    /// [§ 13.2.5.31 Script data double escape end state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escape-end-state)
    ScriptDataDoubleEscapeEnd,
    /// [§ 13.2.5.32 Before attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state)
    BeforeAttributeName,
    /// [§ 13.2.5.33 Attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state)
    AttributeName,
    /// [§ 13.2.5.34 After attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-name-state)
    AfterAttributeName,
    /// [§ 13.2.5.35 Before attribute value state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-value-state)
    BeforeAttributeValue,
    /// [§ 13.2.5.36 Attribute value (double-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state)
    AttributeValueDoubleQuoted,
    /// [§ 13.2.5.37 Attribute value (single-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(single-quoted)-state)
    AttributeValueSingleQuoted,
    /// [§ 13.2.5.38 Attribute value (unquoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(unquoted)-state)
    AttributeValueUnquoted,
    /// [§ 13.2.5.39 After attribute value (quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-value-(quoted)-state)
    AfterAttributeValueQuoted,
    /// [§ 13.2.5.40 Self-closing start tag state](https://html.spec.whatwg.org/multipage/parsing.html#self-closing-start-tag-state)
    SelfClosingStartTag,
    /// [§ 13.2.5.41 Bogus comment state](https://html.spec.whatwg.org/multipage/parsing.html#bogus-comment-state)
    BogusComment,
    /// [§ 13.2.5.42 Markup declaration open state](https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state)
    MarkupDeclarationOpen,
    /// [§ 13.2.5.43 Comment start state](https://html.spec.whatwg.org/multipage/parsing.html#comment-start-state)
    CommentStart,
    /// [§ 13.2.5.44 Comment start dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-start-dash-state)
    CommentStartDash,
    /// [§ 13.2.5.45 Comment state](https://html.spec.whatwg.org/multipage/parsing.html#comment-state)
    Comment,
    /// [§ 13.2.5.46 Comment less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-state)
    CommentLessThanSign,
    /// [§ 13.2.5.47 Comment less-than sign bang state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-state)
    CommentLessThanSignBang,
    /// [§ 13.2.5.48 Comment less-than sign bang dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-state)
    CommentLessThanSignBangDash,
    /// [§ 13.2.5.49 Comment less-than sign bang dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-dash-state)
    CommentLessThanSignBangDashDash,
    /// [§ 13.2.5.50 Comment end dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-dash-state)
    CommentEndDash,
    /// [§ 13.2.5.51 Comment end state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-state)
    CommentEnd,
    /// [§ 13.2.5.52 Comment end bang state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-bang-state)
    CommentEndBang,
    /// [§ 13.2.5.53 DOCTYPE state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-state)
    DOCTYPE,
    /// [§ 13.2.5.54 Before DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-name-state)
    BeforeDOCTYPEName,
    /// [§ 13.2.5.55 DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-name-state)
    DOCTYPEName,
    /// [§ 13.2.5.56 After DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-name-state)
    AfterDOCTYPEName,
    /// [§ 13.2.5.57 After DOCTYPE public keyword state](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-public-keyword-state)
    AfterDOCTYPEPublicKeyword,
    /// [§ 13.2.5.58 Before DOCTYPE public identifier state](https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-public-identifier-state)
    BeforeDOCTYPEPublicIdentifier,
    /// [§ 13.2.5.59 DOCTYPE public identifier (double-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-public-identifier-(double-quoted)-state)
    DOCTYPEPublicIdentifierDoubleQuoted,
    /// [§ 13.2.5.60 DOCTYPE public identifier (single-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-public-identifier-(single-quoted)-state)
    DOCTYPEPublicIdentifierSingleQuoted,
    /// [§ 13.2.5.61 After DOCTYPE public identifier state](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-public-identifier-state)
    AfterDOCTYPEPublicIdentifier,
    /// [§ 13.2.5.62 Between DOCTYPE public and system identifiers state](https://html.spec.whatwg.org/multipage/parsing.html#between-doctype-public-and-system-identifiers-state)
    BetweenDOCTYPEPublicAndSystemIdentifiers,
    /// [§ 13.2.5.63 After DOCTYPE system keyword state](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-system-keyword-state)
    AfterDOCTYPESystemKeyword,
    /// [§ 13.2.5.64 Before DOCTYPE system identifier state](https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-system-identifier-state)
    BeforeDOCTYPESystemIdentifier,
    /// [§ 13.2.5.65 DOCTYPE system identifier (double-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-system-identifier-(double-quoted)-state)
    DOCTYPESystemIdentifierDoubleQuoted,
    /// [§ 13.2.5.66 DOCTYPE system identifier (single-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-system-identifier-(single-quoted)-state)
    DOCTYPESystemIdentifierSingleQuoted,
    /// [§ 13.2.5.67 After DOCTYPE system identifier state](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-system-identifier-state)
    AfterDOCTYPESystemIdentifier,
    /// [§ 13.2.5.68 Bogus DOCTYPE state](https://html.spec.whatwg.org/multipage/parsing.html#bogus-doctype-state)
    BogusDOCTYPE,
    /// [§ 13.2.5.69 CDATA section state](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-state)
    CDATASection,
    /// [§ 13.2.5.70 CDATA section bracket state](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-bracket-state)
    CDATASectionBracket,
    /// [§ 13.2.5.71 CDATA section end state](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-end-state)
    CDATASectionEnd,
    /// [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
    CharacterReference,
    /// [§ 13.2.5.73 Named character reference state](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
    NamedCharacterReference,
    /// [§ 13.2.5.74 Ambiguous ampersand state](https://html.spec.whatwg.org/multipage/parsing.html#ambiguous-ampersand-state)
    AmbiguousAmpersand,
    /// [§ 13.2.5.75 Numeric character reference state](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-state)
    NumericCharacterReference,
    /// [§ 13.2.5.76 Hexadecimal character reference start state](https://html.spec.whatwg.org/multipage/parsing.html#hexadecimal-character-reference-start-state)
    HexadecimalCharacterReferenceStart,
    /// [§ 13.2.5.77 Decimal character reference start state](https://html.spec.whatwg.org/multipage/parsing.html#decimal-character-reference-start-state)
    DecimalCharacterReferenceStart,
    /// [§ 13.2.5.78 Hexadecimal character reference state](https://html.spec.whatwg.org/multipage/parsing.html#hexadecimal-character-reference-state)
    HexadecimalCharacterReference,
    /// [§ 13.2.5.79 Decimal character reference state](https://html.spec.whatwg.org/multipage/parsing.html#decimal-character-reference-state)
    DecimalCharacterReference,
    /// [§ 13.2.5.80 Numeric character reference end state](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state)
    NumericCharacterReferenceEnd,
}

/// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
///
/// "Implementations must act as if they used the following state machine to tokenize HTML."
///
/// This struct maintains the state machine for tokenizing HTML input into tokens.
pub struct HTMLTokenizer {
    pub(super) state: TokenizerState,
    pub(super) return_state: Option<TokenizerState>,
    pub(super) input: String,
    pub(super) current_pos: usize,
    pub(super) current_input_character: Option<char>,
    pub(super) current_token: Option<Token>,
    pub(super) at_eof: bool,
    pub(super) token_stream: Vec<Token>,
    // When true, the next iteration of the main loop will not consume a new character.
    // "Reconsume in the X state" sets this flag.
    pub(super) reconsume: bool,

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    /// "The last start tag token emitted is used as part of the tree construction stage
    /// and in the RCDATA, RAWTEXT, and script data states."
    pub(super) last_start_tag_name: Option<String>,

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#temporary-buffer)
    /// "The temporary buffer is used to temporarily store characters during certain
    /// tokenization operations, particularly for end tag detection in RCDATA/RAWTEXT states."
    pub(super) temporary_buffer: String,
}
impl HTMLTokenizer {
    /// Create a new tokenizer for the given input.
    ///
    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization):
    /// "The tokenizer state machine consists of the states defined in the
    /// following subsections. The initial state is the data state."
    pub fn new(input: String) -> Self {
        // [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
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
            last_start_tag_name: None,
            temporary_buffer: String::new(),
        }
    }

    /// Consume the tokenizer and return the token stream.
    /// Call this after run() to get the tokens for the parser.
    pub fn into_tokens(self) -> Vec<Token> {
        self.token_stream
    }

    /// [§ 13.2.5.1 Data state](https://html.spec.whatwg.org/multipage/parsing.html#data-state)
    fn handle_data_state(&mut self) {
        match self.current_input_character {
            // "U+0026 AMPERSAND (&) - Set the return state to the data state.
            // Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::Data);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // "U+003C LESS-THAN SIGN (<) - Switch to the tag open state."
            Some('<') => {
                self.switch_to(TokenizerState::TagOpen);
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error.
            // Emit the current input character as a character token."
            Some('\0') => {
                self.log_parse_error();
                self.emit_character_token('\0');
                self.switch_to(TokenizerState::Data);
            }
            // "EOF - Emit an end-of-file token."
            None => {
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Emit the current input character as a character token."
            Some(c) => {
                self.emit_character_token(c);
                self.switch_to(TokenizerState::Data);
            }
        }
    }
    /// [§ 13.2.5.2 RCDATA state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-state)
    fn handle_rcdata_state(&mut self) {
        match self.current_input_character {
            // "U+0026 AMPERSAND (&)"
            // "Set the return state to the RCDATA state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::RCDATA);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // "U+003C LESS-THAN SIGN (<)"
            // "Switch to the RCDATA less-than sign state."
            Some('<') => {
                self.switch_to(TokenizerState::RCDATALessThanSign);
            }
            // "U+0000 NULL"
            // "This is an unexpected-null-character parse error. Emit a U+FFFD REPLACEMENT
            // CHARACTER character token."
            Some('\0') => {
                self.log_parse_error();
                self.emit_character_token('\u{FFFD}');
            }
            // "EOF"
            // "Emit an end-of-file token."
            None => {
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else"
            // "Emit the current input character as a character token."
            Some(c) => {
                self.emit_character_token(c);
            }
        }
    }

    /// [§ 13.2.5.9 RCDATA less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-less-than-sign-state)
    fn handle_rcdata_less_than_sign_state(&mut self) {
        match self.current_input_character {
            // "U+002F SOLIDUS (/)"
            // "Set the temporary buffer to the empty string. Switch to the RCDATA end tag open state."
            Some('/') => {
                self.temporary_buffer.clear();
                self.switch_to(TokenizerState::RCDATAEndTagOpen);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token. Reconsume in the RCDATA state."
            _ => {
                self.emit_character_token('<');
                self.reconsume_in(TokenizerState::RCDATA);
            }
        }
    }

    /// [§ 13.2.5.10 RCDATA end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-open-state)
    fn handle_rcdata_end_tag_open_state(&mut self) {
        match self.current_input_character {
            // "ASCII alpha"
            // "Create a new end tag token, set its tag name to the empty string. Reconsume in
            // the RCDATA end tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_end_tag());
                self.reconsume_in(TokenizerState::RCDATAEndTagName);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token and a U+002F SOLIDUS character token.
            // Reconsume in the RCDATA state."
            _ => {
                self.emit_character_token('<');
                self.emit_character_token('/');
                self.reconsume_in(TokenizerState::RCDATA);
            }
        }
    }

    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    fn handle_rcdata_end_tag_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION (tab)"
            // "U+000A LINE FEED (LF)"
            // "U+000C FORM FEED (FF)"
            // "U+0020 SPACE"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // before attribute name state. Otherwise, treat it as per the "anything else" entry below."
            Some(c) if Self::is_whitespace_char(c) => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::BeforeAttributeName);
                } else {
                    self.emit_rcdata_end_tag_name_anything_else();
                }
            }
            // "U+002F SOLIDUS (/)"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // self-closing start tag state. Otherwise, treat it as per the "anything else" entry below."
            Some('/') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::SelfClosingStartTag);
                } else {
                    self.emit_rcdata_end_tag_name_anything_else();
                }
            }
            // "U+003E GREATER-THAN SIGN (>)"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // data state and emit the current tag token. Otherwise, treat it as per the "anything
            // else" entry below."
            Some('>') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::Data);
                    self.emit_token();
                } else {
                    self.emit_rcdata_end_tag_name_anything_else();
                }
            }
            // "ASCII upper alpha"
            // "Append the lowercase version of the current input character (add 0x0020 to the
            // character's code point) to the current tag token's tag name. Append the current
            // input character to the temporary buffer."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
            }
            // "ASCII lower alpha"
            // "Append the current input character to the current tag token's tag name. Append
            // the current input character to the temporary buffer."
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c);
                }
                self.temporary_buffer.push(c);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character token,
            // and a character token for each of the characters in the temporary buffer (in the
            // order they were added to the buffer). Reconsume in the RCDATA state."
            _ => {
                self.emit_rcdata_end_tag_name_anything_else();
            }
        }
    }

    /// [§ 13.2.5.3 RAWTEXT state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-state)
    fn handle_rawtext_state(&mut self) {
        match self.current_input_character {
            // "U+003C LESS-THAN SIGN (<)"
            // "Switch to the RAWTEXT less-than sign state."
            Some('<') => {
                self.switch_to(TokenizerState::RAWTEXTLessThanSign);
            }
            // "U+0000 NULL"
            // "This is an unexpected-null-character parse error. Emit a U+FFFD REPLACEMENT
            // CHARACTER character token."
            Some('\0') => {
                self.log_parse_error();
                self.emit_character_token('\u{FFFD}');
            }
            // "EOF"
            // "Emit an end-of-file token."
            None => {
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else"
            // "Emit the current input character as a character token."
            Some(c) => {
                self.emit_character_token(c);
            }
        }
    }
    /// [§ 13.2.5.4 Script data state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-state)
    fn handle_script_data_state(&mut self) {
        // "Consume the next input character:"
        match self.current_input_character {
            // "U+003C LESS-THAN SIGN (<)"
            // "Switch to the script data less-than sign state."
            Some('<') => {
                self.switch_to(TokenizerState::ScriptDataLessThanSign);
            }
            // "U+0000 NULL"
            // "This is an unexpected-null-character parse error. Emit a U+FFFD REPLACEMENT CHARACTER character token."
            Some('\0') => {
                self.log_parse_error();
                self.emit_character_token('\u{FFFD}');
            }
            // "EOF"
            // "Emit an end-of-file token."
            None => {
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else"
            // "Emit the current input character as a character token."
            Some(c) => {
                self.emit_character_token(c);
            }
        }
    }

    /// [§ 13.2.5.17 Script data less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-less-than-sign-state)
    fn handle_script_data_less_than_sign_state(&mut self) {
        // "Consume the next input character:"
        match self.current_input_character {
            // "U+002F SOLIDUS (/)"
            // "Set the temporary buffer to the empty string. Switch to the script data end tag open state."
            Some('/') => {
                self.temporary_buffer.clear();
                self.switch_to(TokenizerState::ScriptDataEndTagOpen);
            }
            // "U+0021 EXCLAMATION MARK (!)"
            // "Switch to the script data escape start state. Emit a U+003C LESS-THAN SIGN character token
            // and a U+0021 EXCLAMATION MARK character token."
            Some('!') => {
                todo!("implement: switch to ScriptDataEscapeStart, emit '<' and '!'");
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token. Reconsume in the script data state."
            _ => {
                self.emit_character_token('<');
                self.reconsume_in(TokenizerState::ScriptData);
            }
        }
    }

    /// [§ 13.2.5.18 Script data end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-open-state)
    fn handle_script_data_end_tag_open_state(&mut self) {
        // "Consume the next input character:"
        match self.current_input_character {
            // "ASCII alpha"
            // "Create a new end tag token, set its tag name to the empty string. Reconsume in the
            // script data end tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_end_tag());
                self.reconsume_in(TokenizerState::ScriptDataEndTagName);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token and a U+002F SOLIDUS character token.
            // Reconsume in the script data state."
            _ => {
                self.emit_character_token('<');
                self.emit_character_token('/');
                self.reconsume_in(TokenizerState::ScriptData);
            }
        }
    }

    /// [§ 13.2.5.19 Script data end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-name-state)
    fn handle_script_data_end_tag_name_state(&mut self) {
        // "Consume the next input character:"
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION (tab)"
            // "U+000A LINE FEED (LF)"
            // "U+000C FORM FEED (FF)"
            // "U+0020 SPACE"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // before attribute name state. Otherwise, treat it as per the \"anything else\" entry below."
            Some(c) if Self::is_whitespace_char(c) => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::BeforeAttributeName);
                } else {
                    self.emit_script_data_end_tag_name_anything_else();
                }
            }
            // "U+002F SOLIDUS (/)"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // self-closing start tag state. Otherwise, treat it as per the \"anything else\" entry below."
            Some('/') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::SelfClosingStartTag);
                } else {
                    self.emit_script_data_end_tag_name_anything_else();
                }
            }
            // "U+003E GREATER-THAN SIGN (>)"
            // "If the current end tag token is an appropriate end tag token, then switch to the data state
            // and emit the current tag token. Otherwise, treat it as per the \"anything else\" entry below."
            Some('>') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::Data);
                    self.emit_token();
                } else {
                    self.emit_script_data_end_tag_name_anything_else();
                }
            }
            // "ASCII upper alpha"
            // "Append the lowercase version of the current input character to the current tag token's
            // tag name. Append the current input character to the temporary buffer."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
            }
            // "ASCII lower alpha"
            // "Append the current input character to the current tag token's tag name. Append the
            // current input character to the temporary buffer."
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c);
                }
                self.temporary_buffer.push(c);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character token, and a
            // character token for each of the characters in the temporary buffer. Reconsume in the
            // script data state."
            _ => {
                self.emit_script_data_end_tag_name_anything_else();
            }
        }
    }

    /// [§ 13.2.5.12 RAWTEXT less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-less-than-sign-state)
    fn handle_rawtext_less_than_sign_state(&mut self) {
        match self.current_input_character {
            // "U+002F SOLIDUS (/)"
            // "Set the temporary buffer to the empty string. Switch to the RAWTEXT end tag open state."
            Some('/') => {
                self.temporary_buffer.clear();
                self.switch_to(TokenizerState::RAWTEXTEndTagOpen);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token. Reconsume in the RAWTEXT state."
            _ => {
                self.emit_character_token('<');
                self.reconsume_in(TokenizerState::RAWTEXT);
            }
        }
    }

    /// [§ 13.2.5.13 RAWTEXT end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-open-state)
    fn handle_rawtext_end_tag_open_state(&mut self) {
        match self.current_input_character {
            // "ASCII alpha"
            // "Create a new end tag token, set its tag name to the empty string. Reconsume in
            // the RAWTEXT end tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_end_tag());
                self.reconsume_in(TokenizerState::RAWTEXTEndTagName);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token and a U+002F SOLIDUS character token.
            // Reconsume in the RAWTEXT state."
            _ => {
                self.emit_character_token('<');
                self.emit_character_token('/');
                self.reconsume_in(TokenizerState::RAWTEXT);
            }
        }
    }

    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    fn handle_rawtext_end_tag_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION (tab)"
            // "U+000A LINE FEED (LF)"
            // "U+000C FORM FEED (FF)"
            // "U+0020 SPACE"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // before attribute name state. Otherwise, treat it as per the "anything else" entry below."
            Some(c) if Self::is_whitespace_char(c) => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::BeforeAttributeName);
                } else {
                    self.emit_rawtext_end_tag_name_anything_else();
                }
            }
            // "U+002F SOLIDUS (/)"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // self-closing start tag state. Otherwise, treat it as per the "anything else" entry below."
            Some('/') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::SelfClosingStartTag);
                } else {
                    self.emit_rawtext_end_tag_name_anything_else();
                }
            }
            // "U+003E GREATER-THAN SIGN (>)"
            // "If the current end tag token is an appropriate end tag token, then switch to the
            // data state and emit the current tag token. Otherwise, treat it as per the "anything
            // else" entry below."
            Some('>') => {
                if self.is_appropriate_end_tag_token() {
                    self.switch_to(TokenizerState::Data);
                    self.emit_token();
                } else {
                    self.emit_rawtext_end_tag_name_anything_else();
                }
            }
            // "ASCII upper alpha"
            // "Append the lowercase version of the current input character (add 0x0020 to the
            // character's code point) to the current tag token's tag name. Append the current
            // input character to the temporary buffer."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
            }
            // "ASCII lower alpha"
            // "Append the current input character to the current tag token's tag name. Append
            // the current input character to the temporary buffer."
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c);
                }
                self.temporary_buffer.push(c);
            }
            // "Anything else"
            // "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character token,
            // and a character token for each of the characters in the temporary buffer (in the
            // order they were added to the buffer). Reconsume in the RAWTEXT state."
            _ => {
                self.emit_rawtext_end_tag_name_anything_else();
            }
        }
    }

    /// [§ 13.2.5.6 Tag open state](https://html.spec.whatwg.org/multipage/parsing.html#tag-open-state)
    fn handle_tag_open_state(&mut self) {
        match self.current_input_character {
            // "U+0021 EXCLAMATION MARK (!) - Switch to the markup declaration open state."
            // NOTE: We use reconsume_in here so that MarkupDeclarationOpen can peek ahead
            // without the main loop consuming a character first. This state uses lookahead
            // rather than consuming the "current input character".
            Some('!') => {
                self.reconsume_in(TokenizerState::MarkupDeclarationOpen);
            }
            // "U+002F SOLIDUS (/) - Switch to the end tag open state."
            Some('/') => {
                self.switch_to(TokenizerState::EndTagOpen);
            }
            // "ASCII alpha - Create a new start tag token, set its tag name to the empty
            // string. Reconsume in the tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_start_tag());
                self.reconsume_in(TokenizerState::TagName);
            }
            // "U+003F QUESTION MARK (?) - This is an unexpected-question-mark-instead-of-tag-name
            // parse error. Create a comment token whose data is the empty string. Reconsume in the
            // bogus comment state."
            Some('?') => {
                self.log_parse_error();
                self.current_token = Some(Token::new_comment());
                self.reconsume_in(TokenizerState::BogusComment);
            }
            // "EOF - This is an eof-before-tag-name parse error. Emit a U+003C LESS-THAN SIGN
            // character token and an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - This is an invalid-first-character-of-tag-name parse error.
            // Emit a U+003C LESS-THAN SIGN character token. Reconsume in the data state."
            Some(_) => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.reconsume_in(TokenizerState::Data);
            }
        }
    }
    /// [§ 13.2.5.42 Markup declaration open state](https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state)
    fn handle_markup_declaration_open_state(&mut self) {
        // "If the next two characters are both U+002D HYPHEN-MINUS characters (-),
        // consume those two characters, create a comment token whose data is the empty
        // string, and switch to the comment start state."
        if self.next_few_characters_are("--") {
            self.consume_string("--");
            self.current_token = Some(Token::new_comment());
            self.switch_to(TokenizerState::CommentStart);
        }
        // "Otherwise, if the next seven characters are an ASCII case-insensitive
        // match for the word 'DOCTYPE', consume those characters and switch to the
        // DOCTYPE state."
        else if self.next_few_characters_are_case_insensitive("DOCTYPE") {
            self.consume_string("DOCTYPE");
            self.switch_to(TokenizerState::DOCTYPE);
        }
        // "Otherwise, if there is an adjusted current node and it is not an element
        // in the HTML namespace and the next seven characters are a case-sensitive match
        // for the string '[CDATA[', then consume those characters and switch to the
        // CDATA section state."
        else if self.next_few_characters_are("[CDATA[") {
            // TODO: Check adjusted current node condition
            self.consume_string("[CDATA[");
            self.switch_to(TokenizerState::CDATASection);
        }
        // "Otherwise, this is an incorrectly-opened-comment parse error. Create a
        // comment token whose data is the empty string. Switch to the bogus comment state
        // (don't consume anything in the current state)."
        else {
            self.log_parse_error();
            self.current_token = Some(Token::new_comment());
            self.reconsume_in(TokenizerState::BogusComment);
        }
    }
    /// [§ 13.2.5.53 DOCTYPE state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-state)
    fn handle_doctype_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before DOCTYPE name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeDOCTYPEName);
            }
            // "U+003E GREATER-THAN SIGN (>) - Reconsume in the before DOCTYPE name state."
            Some('>') => {
                self.reconsume_in(TokenizerState::BeforeDOCTYPEName);
            }
            // "EOF - This is an eof-in-doctype parse error. Create a new DOCTYPE token.
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
            // "Anything else - This is a missing-whitespace-before-doctype-name parse error.
            // Reconsume in the before DOCTYPE name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeDOCTYPEName);
            }
        }
    }
    /// [§ 13.2.5.54 Before DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#before-doctype-name-state)
    fn handle_before_doctype_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeDOCTYPEName);
            }
            // "ASCII upper alpha - Create a new DOCTYPE token. Set the token's name to
            // the lowercase version of the current input character. Switch to the DOCTYPE name state."
            Some(c) if c.is_ascii_uppercase() => {
                let mut token = Token::new_doctype();
                token.append_to_doctype_name(c.to_ascii_lowercase());
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Create a new
            // DOCTYPE token. Set the token's name to a U+FFFD REPLACEMENT CHARACTER. Switch to
            // the DOCTYPE name state."
            Some('\0') => {
                self.log_parse_error();
                let mut token = Token::new_doctype();
                token.append_to_doctype_name('\u{FFFD}');
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
            // "U+003E GREATER-THAN SIGN (>) - This is a missing-doctype-name parse error.
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
            // "EOF - This is an eof-in-doctype parse error. Create a new DOCTYPE token.
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
            // "Anything else - Create a new DOCTYPE token. Set the token's name to the
            // current input character. Switch to the DOCTYPE name state."
            Some(c) => {
                let mut token = Token::new_doctype();
                token.append_to_doctype_name(c);
                self.current_token = Some(token);
                self.switch_to(TokenizerState::DOCTYPEName);
            }
        }
    }
    /// [§ 13.2.5.55 DOCTYPE name state](https://html.spec.whatwg.org/multipage/parsing.html#doctype-name-state)
    fn handle_doctype_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the after DOCTYPE name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::AfterDOCTYPEName);
            }
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current DOCTYPE token's name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name(c.to_ascii_lowercase());
                }
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current DOCTYPE token's name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name('\u{FFFD}');
                }
            }
            // "EOF - This is an eof-in-doctype parse error. Set the current DOCTYPE token's
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
            // "Anything else - Append the current input character to the current DOCTYPE
            // token's name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_doctype_name(c);
                }
            }
        }
    }
    /// [§ 13.2.5.8 Tag name state](https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state)
    fn handle_tag_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current tag token's tag name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c.to_ascii_lowercase());
                }
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current tag token's tag name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name('\u{FFFD}');
                }
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append the current input character to the current tag
            // token's tag name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_tag_name(c);
                }
            }
        }
    }
    /// [§ 13.2.5.40 Self-closing start tag state](https://html.spec.whatwg.org/multipage/parsing.html#self-closing-start-tag-state)
    fn handle_self_closing_start_tag_state(&mut self) {
        match self.current_input_character {
            // "U+003E GREATER-THAN SIGN (>) - Set the self-closing flag of the current
            // tag token. Switch to the data state. Emit the current token."
            Some('>') => {
                if let Some(ref mut token) = self.current_token {
                    token.set_self_closing();
                }
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - This is an unexpected-solidus-in-tag parse error.
            // Reconsume in the before attribute name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeAttributeName);
            }
        }
    }
    /// [§ 13.2.5.7 End tag open state](https://html.spec.whatwg.org/multipage/parsing.html#end-tag-open-state)
    fn handle_end_tag_open_state(&mut self) {
        match self.current_input_character {
            // "ASCII alpha - Create a new end tag token, set its tag name to the empty
            // string. Reconsume in the tag name state."
            Some(c) if c.is_ascii_alphabetic() => {
                self.current_token = Some(Token::new_end_tag());
                self.reconsume_in(TokenizerState::TagName);
            }
            // "U+003E GREATER-THAN SIGN (>) - This is a missing-end-tag-name parse error.
            // Switch to the data state."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
            }
            // "EOF - This is an eof-before-tag-name parse error. Emit a U+003C LESS-THAN
            // SIGN character token, a U+002F SOLIDUS character token and an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_character_token('<');
                self.emit_character_token('/');
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - This is an invalid-first-character-of-tag-name parse error.
            // Create a comment token whose data is the empty string. Reconsume in the bogus
            // comment state."
            Some(_) => {
                self.log_parse_error();
                self.current_token = Some(Token::new_comment());
                self.reconsume_in(TokenizerState::BogusComment);
            }
        }
    }

    /// [§ 13.2.5.32 Before attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state)
    fn handle_before_attribute_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // "U+002F SOLIDUS (/), U+003E GREATER-THAN SIGN (>), EOF -
            // Reconsume in the after attribute name state."
            Some('/') | Some('>') | None => {
                self.reconsume_in(TokenizerState::AfterAttributeName);
            }
            // "U+003D EQUALS SIGN (=) - This is an unexpected-equals-sign-before-attribute-name
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
            // "Anything else - Start a new attribute in the current tag token. Set that
            // attribute name and value to the empty string. Reconsume in the attribute name state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.start_new_attribute();
                }
                self.reconsume_in(TokenizerState::AttributeName);
            }
        }
    }

    /// [§ 13.2.5.33 Attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state)
    fn handle_attribute_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
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
            // "U+003D EQUALS SIGN (=) - Switch to the before attribute value state."
            Some('=') => {
                self.check_duplicate_attribute();
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // "ASCII upper alpha - Append the lowercase version of the current input
            // character to the current attribute's name."
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(c.to_ascii_lowercase());
                }
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's name."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name('\u{FFFD}');
                }
            }
            // "U+0022 QUOTATION MARK (\"), U+0027 APOSTROPHE ('), U+003C LESS-THAN SIGN (<) -
            // This is an unexpected-character-in-attribute-name parse error. Treat it as per the
            // 'anything else' entry below."
            Some('"') | Some('\'') | Some('<') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(self.current_input_character.unwrap());
                }
            }
            // "Anything else - Append the current input character to the current attribute's name."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_name(c);
                }
            }
        }
    }

    /// [§ 13.2.5.34 After attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-name-state)
    fn handle_after_attribute_name_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::AfterAttributeName);
            }
            // "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // "U+003D EQUALS SIGN (=) - Switch to the before attribute value state."
            Some('=') => {
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Start a new attribute in the current tag token. Set that
            // attribute name and value to the empty string. Reconsume in the attribute name state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.start_new_attribute();
                }
                self.reconsume_in(TokenizerState::AttributeName);
            }
        }
    }

    /// [§ 13.2.5.35 Before attribute value state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-value-state)
    fn handle_before_attribute_value_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Ignore the character."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeValue);
            }
            // "U+0022 QUOTATION MARK (\") - Switch to the attribute value (double-quoted) state."
            Some('"') => {
                self.switch_to(TokenizerState::AttributeValueDoubleQuoted);
            }
            // "U+0027 APOSTROPHE (') - Switch to the attribute value (single-quoted) state."
            Some('\'') => {
                self.switch_to(TokenizerState::AttributeValueSingleQuoted);
            }
            // "U+003E GREATER-THAN SIGN (>) - This is a missing-attribute-value parse error.
            // Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "Anything else - Reconsume in the attribute value (unquoted) state."
            _ => {
                self.reconsume_in(TokenizerState::AttributeValueUnquoted);
            }
        }
    }

    /// [§ 13.2.5.36 Attribute value (double-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state)
    fn handle_attribute_value_double_quoted_state(&mut self) {
        match self.current_input_character {
            // "U+0022 QUOTATION MARK (\") - Switch to the after attribute value (quoted) state."
            Some('"') => {
                self.switch_to(TokenizerState::AfterAttributeValueQuoted);
            }
            // "U+0026 AMPERSAND (&) - Set the return state to the attribute value (double-quoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueDoubleQuoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    /// [§ 13.2.5.37 Attribute value (single-quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(single-quoted)-state)
    fn handle_attribute_value_single_quoted_state(&mut self) {
        match self.current_input_character {
            // "U+0027 APOSTROPHE (') - Switch to the after attribute value (quoted) state."
            Some('\'') => {
                self.switch_to(TokenizerState::AfterAttributeValueQuoted);
            }
            // "U+0026 AMPERSAND (&) - Set the return state to the attribute value (single-quoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueSingleQuoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    /// [§ 13.2.5.38 Attribute value (unquoted) state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(unquoted)-state)
    fn handle_attribute_value_unquoted_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // "U+0026 AMPERSAND (&) - Set the return state to the attribute value (unquoted)
            // state. Switch to the character reference state."
            Some('&') => {
                self.return_state = Some(TokenizerState::AttributeValueUnquoted);
                self.switch_to(TokenizerState::CharacterReference);
            }
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER to the current attribute's value."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value('\u{FFFD}');
                }
            }
            // "U+0022 QUOTATION MARK (\"), U+0027 APOSTROPHE ('), U+003C LESS-THAN SIGN (<),
            // U+003D EQUALS SIGN (=), U+0060 GRAVE ACCENT (`) - This is an
            // unexpected-character-in-unquoted-attribute-value parse error. Treat it as per the
            // 'anything else' entry below."
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(self.current_input_character.unwrap());
                }
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append the current input character to the current attribute's value."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        }
    }

    /// [§ 13.2.5.39 After attribute value (quoted) state](https://html.spec.whatwg.org/multipage/parsing.html#after-attribute-value-(quoted)-state)
    fn handle_after_attribute_value_quoted_state(&mut self) {
        match self.current_input_character {
            // "U+0009 CHARACTER TABULATION, U+000A LINE FEED, U+000C FORM FEED,
            // U+0020 SPACE - Switch to the before attribute name state."
            Some(c) if Self::is_whitespace_char(c) => {
                self.switch_to(TokenizerState::BeforeAttributeName);
            }
            // "U+002F SOLIDUS (/) - Switch to the self-closing start tag state."
            Some('/') => {
                self.switch_to(TokenizerState::SelfClosingStartTag);
            }
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current tag token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - This is an eof-in-tag parse error. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - This is a missing-whitespace-between-attributes parse error.
            // Reconsume in the before attribute name state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::BeforeAttributeName);
            }
        }
    }

    /// [§ 13.2.5.43 Comment start state](https://html.spec.whatwg.org/multipage/parsing.html#comment-start-state)
    fn handle_comment_start_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment start dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentStartDash);
            }
            // "U+003E GREATER-THAN SIGN (>) - This is an abrupt-closing-of-empty-comment
            // parse error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    /// [§ 13.2.5.44 Comment start dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-start-dash-state)
    fn handle_comment_start_dash_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment end state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEnd);
            }
            // "U+003E GREATER-THAN SIGN (>) - This is an abrupt-closing-of-empty-comment
            // parse error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append a U+002D HYPHEN-MINUS character (-) to the comment
            // token's data. Reconsume in the comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    /// [§ 13.2.5.45 Comment state](https://html.spec.whatwg.org/multipage/parsing.html#comment-state)
    fn handle_comment_state(&mut self) {
        match self.current_input_character {
            // "U+003C LESS-THAN SIGN (<) - Append the current input character to the
            // comment token's data. Switch to the comment less-than sign state."
            Some('<') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('<');
                }
                self.switch_to(TokenizerState::CommentLessThanSign);
            }
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment end dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEndDash);
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER character to the comment token's data."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('\u{FFFD}');
                }
            }
            // "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append the current input character to the comment token's data."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment(c);
                }
            }
        }
    }

    /// [§ 13.2.5.46 Comment less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-state)
    fn handle_comment_less_than_sign_state(&mut self) {
        match self.current_input_character {
            // "U+0021 EXCLAMATION MARK (!) - Append the current input character to the
            // comment token's data. Switch to the comment less-than sign bang state."
            Some('!') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('!');
                }
                self.switch_to(TokenizerState::CommentLessThanSignBang);
            }
            // "U+003C LESS-THAN SIGN (<) - Append the current input character to the
            // comment token's data."
            Some('<') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('<');
                }
            }
            // "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    /// [§ 13.2.5.47 Comment less-than sign bang state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-state)
    fn handle_comment_less_than_sign_bang_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment less-than sign bang dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentLessThanSignBangDash);
            }
            // "Anything else - Reconsume in the comment state."
            _ => {
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    /// [§ 13.2.5.48 Comment less-than sign bang dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-state)
    fn handle_comment_less_than_sign_bang_dash_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment less-than sign bang dash dash state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentLessThanSignBangDashDash);
            }
            // "Anything else - Reconsume in the comment end dash state."
            _ => {
                self.reconsume_in(TokenizerState::CommentEndDash);
            }
        }
    }

    /// [§ 13.2.5.49 Comment less-than sign bang dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-less-than-sign-bang-dash-dash-state)
    fn handle_comment_less_than_sign_bang_dash_dash_state(&mut self) {
        match self.current_input_character {
            // "U+003E GREATER-THAN SIGN (>) - Reconsume in the comment end state."
            Some('>') => {
                self.reconsume_in(TokenizerState::CommentEnd);
            }
            // "EOF - Reconsume in the comment end state."
            None => {
                self.reconsume_in(TokenizerState::CommentEnd);
            }
            // "Anything else - This is a nested-comment parse error. Reconsume in the
            // comment end state."
            Some(_) => {
                self.log_parse_error();
                self.reconsume_in(TokenizerState::CommentEnd);
            }
        }
    }

    /// [§ 13.2.5.50 Comment end dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-dash-state)
    fn handle_comment_end_dash_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Switch to the comment end state."
            Some('-') => {
                self.switch_to(TokenizerState::CommentEnd);
            }
            // "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append a U+002D HYPHEN-MINUS character (-) to the comment
            // token's data. Reconsume in the comment state."
            Some(_) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
                self.reconsume_in(TokenizerState::Comment);
            }
        }
    }

    /// [§ 13.2.5.51 Comment end state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-state)
    fn handle_comment_end_state(&mut self) {
        match self.current_input_character {
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current
            // comment token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "U+0021 EXCLAMATION MARK (!) - Switch to the comment end bang state."
            Some('!') => {
                self.switch_to(TokenizerState::CommentEndBang);
            }
            // "U+002D HYPHEN-MINUS (-) - Append a U+002D HYPHEN-MINUS character (-) to
            // the comment token's data."
            Some('-') => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('-');
                }
            }
            // "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append two U+002D HYPHEN-MINUS characters (-) to the
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

    /// [§ 13.2.5.52 Comment end bang state](https://html.spec.whatwg.org/multipage/parsing.html#comment-end-bang-state)
    fn handle_comment_end_bang_state(&mut self) {
        match self.current_input_character {
            // "U+002D HYPHEN-MINUS (-) - Append two U+002D HYPHEN-MINUS characters (-)
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
            // "U+003E GREATER-THAN SIGN (>) - This is an incorrectly-closed-comment parse
            // error. Switch to the data state. Emit the current comment token."
            Some('>') => {
                self.log_parse_error();
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - This is an eof-in-comment parse error. Emit the current comment
            // token. Emit an end-of-file token."
            None => {
                self.log_parse_error();
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "Anything else - Append two U+002D HYPHEN-MINUS characters (-) and a U+0021
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

    /// [§ 13.2.5.41 Bogus comment state](https://html.spec.whatwg.org/multipage/parsing.html#bogus-comment-state)
    fn handle_bogus_comment_state(&mut self) {
        match self.current_input_character {
            // "U+003E GREATER-THAN SIGN (>) - Switch to the data state. Emit the current
            // comment token."
            Some('>') => {
                self.switch_to(TokenizerState::Data);
                self.emit_token();
            }
            // "EOF - Emit the comment. Emit an end-of-file token."
            None => {
                self.emit_token();
                self.emit_eof_token();
                self.at_eof = true;
            }
            // "U+0000 NULL - This is an unexpected-null-character parse error. Append a
            // U+FFFD REPLACEMENT CHARACTER character to the comment token's data."
            Some('\0') => {
                self.log_parse_error();
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment('\u{FFFD}');
                }
            }
            // "Anything else - Append the current input character to the comment token's data."
            Some(c) => {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_comment(c);
                }
            }
        }
    }

    /// [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
    fn handle_character_reference_state(&mut self) {
        // "Set the temporary buffer to the empty string."
        self.temporary_buffer.clear();
        // "Append a U+0026 AMPERSAND character (&) to the temporary buffer."
        self.temporary_buffer.push('&');

        match self.current_input_character {
            // "ASCII alphanumeric"
            // "Reconsume in the named character reference state."
            Some(c) if c.is_ascii_alphanumeric() => {
                self.reconsume_in(TokenizerState::NamedCharacterReference);
            }
            // "U+0023 NUMBER SIGN (#)"
            // "Append the current input character to the temporary buffer.
            // Switch to the numeric character reference state."
            Some('#') => {
                self.temporary_buffer.push('#');
                self.switch_to(TokenizerState::NumericCharacterReference);
            }
            // "Anything else"
            // "Flush code points consumed as a character reference.
            // Reconsume in the return state."
            _ => {
                self.flush_code_points_consumed_as_character_reference();
                let return_state = self.return_state.take().unwrap();
                self.reconsume_in(return_state);
            }
        }
    }
    /// [§ 13.2.5.73 Named character reference state](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
    fn handle_named_character_reference_state(&mut self) {
        use super::named_character_references::{any_entity_has_prefix, lookup_entity};

        // "Consume the maximum number of characters possible, where the consumed
        // characters are one of the identifiers in the first column of the named
        // character references table. Append each character to the temporary buffer
        // when it's consumed."
        //
        // We enter this state via reconsume, so current_input_character is the first
        // alphanumeric. The temporary_buffer already contains "&" from CharacterReference.

        let mut longest_match: Option<(usize, &'static str)> = None;

        // Process current_input_character first (it was reconsumed, so hasn't been
        // added to buffer yet)
        if let Some(c) = self.current_input_character {
            self.temporary_buffer.push(c);

            // Check for match
            let entity_name = &self.temporary_buffer[1..]; // Skip leading '&'
            if let Some(replacement) = lookup_entity(entity_name) {
                longest_match = Some((self.temporary_buffer.len(), replacement));
            }
        }

        // Keep consuming characters while they could be part of an entity name
        loop {
            let entity_name = &self.temporary_buffer[1..];

            // Stop if we ended with semicolon
            if entity_name.ends_with(';') {
                break;
            }

            // Stop if no entity could start with this prefix
            if !any_entity_has_prefix(entity_name) {
                break;
            }

            // Consume the next character
            let next = self.consume();
            match next {
                Some(c) if c.is_ascii_alphanumeric() || c == ';' => {
                    // "Append each character to the temporary buffer when it's consumed."
                    self.temporary_buffer.push(c);

                    // Check for match
                    let entity_name = &self.temporary_buffer[1..];
                    if let Some(replacement) = lookup_entity(entity_name) {
                        longest_match = Some((self.temporary_buffer.len(), replacement));
                    }
                }
                _ => {
                    // Hit a non-entity character or EOF - need to reconsume it
                    self.current_input_character = next;
                    self.reconsume = true;
                    break;
                }
            }
        }

        // "If there is a match:"
        if let Some((match_len, replacement)) = longest_match {
            let matched_entity = &self.temporary_buffer[1..match_len];
            let last_char_is_semicolon = matched_entity.ends_with(';');

            // "If the character reference was consumed as part of an attribute, and
            // the last character matched is not a U+003B SEMICOLON character (;), and
            // the next input character is either a U+003D EQUALS SIGN character (=) or
            // an ASCII alphanumeric, then, for historical reasons, flush code points
            // consumed as a character reference. Switch to the return state."
            if self.is_consumed_as_part_of_attribute() && !last_char_is_semicolon {
                // The "next input character" is either:
                // - A character we consumed past the match (in buffer after match_len)
                // - current_input_character if we set reconsume
                // - The next char in input if we haven't consumed it
                let next_char = if match_len < self.temporary_buffer.len() {
                    self.temporary_buffer.chars().nth(match_len)
                } else if self.reconsume {
                    self.current_input_character
                } else {
                    self.peek_codepoint(0)
                };

                if matches!(next_char, Some('='))
                    || matches!(next_char, Some(c) if c.is_ascii_alphanumeric())
                {
                    // Historical exception: don't decode, flush as-is
                    self.flush_code_points_consumed_as_character_reference();
                    let return_state = self.return_state.take().unwrap();
                    if self.reconsume {
                        self.state = return_state;
                    } else {
                        self.switch_to(return_state);
                    }
                    return;
                }
            }

            // "If the last character matched is not a U+003B SEMICOLON character (;),
            // then this is a missing-semicolon-after-character-reference parse error."
            if !last_char_is_semicolon {
                self.log_parse_error();
            }

            // Handle any characters we consumed AFTER the match
            let chars_after_match: String = self.temporary_buffer[match_len..].to_string();

            // "Set the temporary buffer to the empty string. Append one or two characters
            // corresponding to the character reference name to the temporary buffer."
            self.temporary_buffer.clear();
            self.temporary_buffer.push_str(replacement);

            // "Flush code points consumed as a character reference."
            self.flush_code_points_consumed_as_character_reference();

            // Emit/append the characters that came after the match
            for c in chars_after_match.chars() {
                if self.is_consumed_as_part_of_attribute() {
                    if let Some(ref mut token) = self.current_token {
                        token.append_to_current_attribute_value(c);
                    }
                } else {
                    self.emit_character_token(c);
                }
            }

            // "Switch to the return state."
            let return_state = self.return_state.take().unwrap();
            if self.reconsume {
                self.state = return_state;
            } else {
                self.switch_to(return_state);
            }
        } else {
            // "Otherwise:" (no match found)
            // "Flush code points consumed as a character reference."
            // The buffer contains "&" plus all characters we consumed.
            self.flush_code_points_consumed_as_character_reference();

            // "Switch to the ambiguous ampersand state."
            if self.reconsume {
                self.state = TokenizerState::AmbiguousAmpersand;
            } else {
                self.switch_to(TokenizerState::AmbiguousAmpersand);
            }
        }
    }

    /// [§ 13.2.5.74 Ambiguous ampersand state](https://html.spec.whatwg.org/multipage/parsing.html#ambiguous-ampersand-state)
    fn handle_ambiguous_ampersand_state(&mut self) {
        match self.current_input_character {
            // "ASCII alphanumeric"
            // "If the character reference was consumed as part of an attribute, then
            // append the current input character to the current attribute's value.
            // Otherwise, emit the current input character as a character token."
            Some(c) if c.is_ascii_alphanumeric() => {
                if self.is_consumed_as_part_of_attribute() {
                    if let Some(ref mut token) = self.current_token {
                        token.append_to_current_attribute_value(c);
                    }
                } else {
                    self.emit_character_token(c);
                }
            }
            // "U+003B SEMICOLON (;)"
            // "This is an unknown-named-character-reference parse error.
            // Reconsume in the return state."
            Some(';') => {
                self.log_parse_error();
                let return_state = self.return_state.take().unwrap();
                self.reconsume_in(return_state);
            }
            // "Anything else"
            // "Reconsume in the return state."
            _ => {
                let return_state = self.return_state.take().unwrap();
                self.reconsume_in(return_state);
            }
        }
    }
    fn handle_numeric_character_reference_state(&mut self) {
        // Consume the next input character:
        match self.current_input_character {
            // ASCII alphanumeric
            // If the character reference was consumed as part of an attribute, then append the current input character to the current attribute's value. Otherwise, emit the current input character as a character token.
            Some(c) if c.is_ascii_alphanumeric() => {
                if self.is_consumed_as_part_of_attribute() {
                    if let Some(ref mut token) = self.current_token {
                        token.append_to_current_attribute_value(c);
                    }
                } else {
                    self.emit_character_token(c);
                }
            }
            // U+003B SEMICOLON (;)
            // This is an unknown-named-character-reference parse error. Reconsume in the return state.
            Some(';') => {
                self.log_parse_error();
                let return_state = self.return_state.take().unwrap();
                self.reconsume_in(return_state);
            }
            // Anything else
            // Reconsume in the return state.
            _ => {
                let return_state = self.return_state.take().unwrap();
                self.reconsume_in(return_state);
            }
        }
    }

    /// Run the tokenizer to completion.
    ///
    /// Processes the input and populates the token stream.
    pub fn run(&mut self) {
        loop {
            // Each state begins by consuming the next input character,
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
                TokenizerState::RCDATA => {
                    self.handle_rcdata_state();
                    continue;
                }
                TokenizerState::RAWTEXT => {
                    self.handle_rawtext_state();
                    continue;
                }
                TokenizerState::ScriptData => {
                    self.handle_script_data_state();
                    continue;
                }
                TokenizerState::PLAINTEXT => {
                    // [§ 13.2.5.5 PLAINTEXT state](https://html.spec.whatwg.org/multipage/parsing.html#plaintext-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Emit a U+FFFD
                    //    REPLACEMENT CHARACTER character token."
                    //
                    // "EOF"
                    //   "Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Emit the current input character as a character token."
                    todo!("PLAINTEXT state")
                }
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
                TokenizerState::RCDATALessThanSign => {
                    self.handle_rcdata_less_than_sign_state();
                    continue;
                }
                TokenizerState::RCDATAEndTagOpen => {
                    self.handle_rcdata_end_tag_open_state();
                    continue;
                }
                TokenizerState::RCDATAEndTagName => {
                    self.handle_rcdata_end_tag_name_state();
                    continue;
                }
                TokenizerState::RAWTEXTLessThanSign => {
                    self.handle_rawtext_less_than_sign_state();
                    continue;
                }
                TokenizerState::RAWTEXTEndTagOpen => {
                    self.handle_rawtext_end_tag_open_state();
                    continue;
                }
                TokenizerState::RAWTEXTEndTagName => {
                    self.handle_rawtext_end_tag_name_state();
                    continue;
                }
                TokenizerState::ScriptDataLessThanSign => {
                    self.handle_script_data_less_than_sign_state();
                }
                TokenizerState::ScriptDataEndTagOpen => {
                    self.handle_script_data_end_tag_open_state();
                }
                TokenizerState::ScriptDataEndTagName => {
                    self.handle_script_data_end_tag_name_state();
                }
                TokenizerState::ScriptDataEscapeStart => {
                    // [§ 13.2.5.18 Script data escape start state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escape-start-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data escape start dash state. Emit a U+002D
                    //    HYPHEN-MINUS character token."
                    //
                    // "Anything else"
                    //   "Reconsume in the script data state."
                    todo!("Script data escape start state")
                }
                TokenizerState::ScriptDataEscapeStartDash => {
                    // [§ 13.2.5.19 Script data escape start dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escape-start-dash-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data escaped dash dash state. Emit a U+002D
                    //    HYPHEN-MINUS character token."
                    //
                    // "Anything else"
                    //   "Reconsume in the script data state."
                    todo!("Script data escape start dash state")
                }
                TokenizerState::ScriptDataEscaped => {
                    // [§ 13.2.5.20 Script data escaped state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data escaped dash state. Emit a U+002D
                    //    HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data escaped less-than sign state."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Emit a U+FFFD
                    //    REPLACEMENT CHARACTER character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Emit the current input character as a character token."
                    todo!("Script data escaped state")
                }
                TokenizerState::ScriptDataEscapedDash => {
                    // [§ 13.2.5.21 Script data escaped dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-dash-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data escaped dash dash state. Emit a U+002D
                    //    HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data escaped less-than sign state."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Switch to the
                    //    script data escaped state. Emit a U+FFFD REPLACEMENT CHARACTER
                    //    character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Switch to the script data escaped state. Emit the current input
                    //    character as a character token."
                    todo!("Script data escaped dash state")
                }
                TokenizerState::ScriptDataEscapedDashDash => {
                    // [§ 13.2.5.22 Script data escaped dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-dash-dash-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Emit a U+002D HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data escaped less-than sign state."
                    //
                    // "U+003E GREATER-THAN SIGN (>)"
                    //   "Switch to the script data state. Emit a U+003E GREATER-THAN SIGN
                    //    character token."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Switch to the
                    //    script data escaped state. Emit a U+FFFD REPLACEMENT CHARACTER
                    //    character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Switch to the script data escaped state. Emit the current input
                    //    character as a character token."
                    todo!("Script data escaped dash dash state")
                }
                TokenizerState::ScriptDataEscapedLessThanSign => {
                    // [§ 13.2.5.23 Script data escaped less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-less-than-sign-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002F SOLIDUS (/)"
                    //   "Set the temporary buffer to the empty string. Switch to the
                    //    script data escaped end tag open state."
                    //
                    // "ASCII alpha"
                    //   "Set the temporary buffer to the empty string. Emit a U+003C
                    //    LESS-THAN SIGN character token. Reconsume in the script data
                    //    double escape start state."
                    //
                    // "Anything else"
                    //   "Emit a U+003C LESS-THAN SIGN character token. Reconsume in the
                    //    script data escaped state."
                    todo!("Script data escaped less-than sign state")
                }
                TokenizerState::ScriptDataEscapedEndTagOpen => {
                    // [§ 13.2.5.24 Script data escaped end tag open state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-end-tag-open-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "ASCII alpha"
                    //   "Create a new end tag token, set its tag name to the empty string.
                    //    Reconsume in the script data escaped end tag name state."
                    //
                    // "Anything else"
                    //   "Emit a U+003C LESS-THAN SIGN character token and a U+002F SOLIDUS
                    //    character token. Reconsume in the script data escaped state."
                    todo!("Script data escaped end tag open state")
                }
                TokenizerState::ScriptDataEscapedEndTagName => {
                    // [§ 13.2.5.25 Script data escaped end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-end-tag-name-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+0009 CHARACTER TABULATION (tab)"
                    // "U+000A LINE FEED (LF)"
                    // "U+000C FORM FEED (FF)"
                    // "U+0020 SPACE"
                    //   "If the current end tag token is an appropriate end tag token,
                    //    then switch to the before attribute name state. Otherwise,
                    //    treat it as per the 'anything else' entry below."
                    //
                    // "U+002F SOLIDUS (/)"
                    //   "If the current end tag token is an appropriate end tag token,
                    //    then switch to the self-closing start tag state. Otherwise,
                    //    treat it as per the 'anything else' entry below."
                    //
                    // "U+003E GREATER-THAN SIGN (>)"
                    //   "If the current end tag token is an appropriate end tag token,
                    //    then switch to the data state and emit the current tag token.
                    //    Otherwise, treat it as per the 'anything else' entry below."
                    //
                    // "ASCII upper alpha"
                    //   "Append the lowercase version of the current input character to
                    //    the current tag token's tag name. Append the current input
                    //    character to the temporary buffer."
                    //
                    // "ASCII lower alpha"
                    //   "Append the current input character to the current tag token's
                    //    tag name. Append the current input character to the temporary
                    //    buffer."
                    //
                    // "Anything else"
                    //   "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS
                    //    character token, and a character token for each of the characters
                    //    in the temporary buffer (in the order they were added to the
                    //    buffer). Reconsume in the script data escaped state."
                    todo!("Script data escaped end tag name state")
                }
                TokenizerState::ScriptDataDoubleEscapeStart => {
                    // [§ 13.2.5.26 Script data double escape start state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escape-start-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+0009 CHARACTER TABULATION (tab)"
                    // "U+000A LINE FEED (LF)"
                    // "U+000C FORM FEED (FF)"
                    // "U+0020 SPACE"
                    // "U+002F SOLIDUS (/)"
                    // "U+003E GREATER-THAN SIGN (>)"
                    //   "If the temporary buffer is the string 'script', then switch to the
                    //    script data double escaped state. Otherwise, switch to the script
                    //    data escaped state. Emit the current input character as a character
                    //    token."
                    //
                    // "ASCII upper alpha"
                    //   "Append the lowercase version of the current input character to the
                    //    temporary buffer. Emit the current input character as a character
                    //    token."
                    //
                    // "ASCII lower alpha"
                    //   "Append the current input character to the temporary buffer. Emit
                    //    the current input character as a character token."
                    //
                    // "Anything else"
                    //   "Reconsume in the script data escaped state."
                    todo!("Script data double escape start state")
                }
                TokenizerState::ScriptDataDoubleEscaped => {
                    // [§ 13.2.5.27 Script data double escaped state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data double escaped dash state. Emit a U+002D
                    //    HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data double escaped less-than sign state. Emit
                    //    a U+003C LESS-THAN SIGN character token."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Emit a U+FFFD
                    //    REPLACEMENT CHARACTER character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Emit the current input character as a character token."
                    todo!("Script data double escaped state")
                }
                TokenizerState::ScriptDataDoubleEscapedDash => {
                    // [§ 13.2.5.28 Script data double escaped dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-dash-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Switch to the script data double escaped dash dash state. Emit a
                    //    U+002D HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data double escaped less-than sign state. Emit
                    //    a U+003C LESS-THAN SIGN character token."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Switch to the
                    //    script data double escaped state. Emit a U+FFFD REPLACEMENT
                    //    CHARACTER character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Switch to the script data double escaped state. Emit the current
                    //    input character as a character token."
                    todo!("Script data double escaped dash state")
                }
                TokenizerState::ScriptDataDoubleEscapedDashDash => {
                    // [§ 13.2.5.29 Script data double escaped dash dash state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-dash-dash-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002D HYPHEN-MINUS (-)"
                    //   "Emit a U+002D HYPHEN-MINUS character token."
                    //
                    // "U+003C LESS-THAN SIGN (<)"
                    //   "Switch to the script data double escaped less-than sign state. Emit
                    //    a U+003C LESS-THAN SIGN character token."
                    //
                    // "U+003E GREATER-THAN SIGN (>)"
                    //   "Switch to the script data state. Emit a U+003E GREATER-THAN SIGN
                    //    character token."
                    //
                    // "U+0000 NULL"
                    //   "This is an unexpected-null-character parse error. Switch to the
                    //    script data double escaped state. Emit a U+FFFD REPLACEMENT
                    //    CHARACTER character token."
                    //
                    // "EOF"
                    //   "This is an eof-in-script-html-comment-like-text parse error.
                    //    Emit an end-of-file token."
                    //
                    // "Anything else"
                    //   "Switch to the script data double escaped state. Emit the current
                    //    input character as a character token."
                    todo!("Script data double escaped dash dash state")
                }
                TokenizerState::ScriptDataDoubleEscapedLessThanSign => {
                    // [§ 13.2.5.30 Script data double escaped less-than sign state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escaped-less-than-sign-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+002F SOLIDUS (/)"
                    //   "Set the temporary buffer to the empty string. Switch to the script
                    //    data double escape end state. Emit a U+002F SOLIDUS character token."
                    //
                    // "Anything else"
                    //   "Reconsume in the script data double escaped state."
                    todo!("Script data double escaped less-than sign state")
                }
                TokenizerState::ScriptDataDoubleEscapeEnd => {
                    // [§ 13.2.5.31 Script data double escape end state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-double-escape-end-state)
                    //
                    // "Consume the next input character:"
                    //
                    // "U+0009 CHARACTER TABULATION (tab)"
                    // "U+000A LINE FEED (LF)"
                    // "U+000C FORM FEED (FF)"
                    // "U+0020 SPACE"
                    // "U+002F SOLIDUS (/)"
                    // "U+003E GREATER-THAN SIGN (>)"
                    //   "If the temporary buffer is the string 'script', then switch to the
                    //    script data escaped state. Otherwise, switch to the script data
                    //    double escaped state. Emit the current input character as a
                    //    character token."
                    //
                    // "ASCII upper alpha"
                    //   "Append the lowercase version of the current input character to the
                    //    temporary buffer. Emit the current input character as a character
                    //    token."
                    //
                    // "ASCII lower alpha"
                    //   "Append the current input character to the temporary buffer. Emit
                    //    the current input character as a character token."
                    //
                    // "Anything else"
                    //   "Reconsume in the script data double escaped state."
                    todo!("Script data double escape end state")
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
                // ===== DOCTYPE IDENTIFIER STATES =====
                // [§ 13.2.5.55-67](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-name-state)
                //
                // These states handle PUBLIC and SYSTEM identifiers in DOCTYPE declarations:
                //   <!DOCTYPE html PUBLIC "..." "...">
                //   <!DOCTYPE html SYSTEM "...">
                //
                // TODO: Implement DOCTYPE identifier parsing in this order:
                //
                // STEP 1: AfterDOCTYPEName - look for PUBLIC/SYSTEM keywords
                //   [§ 13.2.5.55](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-name-state)
                TokenizerState::AfterDOCTYPEName => {
                    todo!("AfterDOCTYPEName state - see STEP 1 above")
                }

                // STEP 2: PUBLIC keyword states - parse public identifier
                //   [§ 13.2.5.56-60](https://html.spec.whatwg.org/multipage/parsing.html#after-doctype-public-keyword-state)
                TokenizerState::AfterDOCTYPEPublicKeyword => {
                    todo!("AfterDOCTYPEPublicKeyword state - see STEP 2")
                }
                TokenizerState::BeforeDOCTYPEPublicIdentifier => {
                    todo!("BeforeDOCTYPEPublicIdentifier state - see STEP 2")
                }
                TokenizerState::DOCTYPEPublicIdentifierDoubleQuoted => {
                    todo!("DOCTYPEPublicIdentifierDoubleQuoted state - see STEP 2")
                }
                TokenizerState::DOCTYPEPublicIdentifierSingleQuoted => {
                    todo!("DOCTYPEPublicIdentifierSingleQuoted state - see STEP 2")
                }
                TokenizerState::AfterDOCTYPEPublicIdentifier => {
                    todo!("AfterDOCTYPEPublicIdentifier state - see STEP 2")
                }

                // STEP 3: Between identifiers and SYSTEM keyword states
                //   [§ 13.2.5.61-67](https://html.spec.whatwg.org/multipage/parsing.html#between-doctype-public-and-system-identifiers-state)
                TokenizerState::BetweenDOCTYPEPublicAndSystemIdentifiers => {
                    todo!("BetweenDOCTYPEPublicAndSystemIdentifiers state - see STEP 3")
                }
                TokenizerState::AfterDOCTYPESystemKeyword => {
                    todo!("AfterDOCTYPESystemKeyword state - see STEP 3")
                }
                TokenizerState::BeforeDOCTYPESystemIdentifier => {
                    todo!("BeforeDOCTYPESystemIdentifier state - see STEP 3")
                }
                TokenizerState::DOCTYPESystemIdentifierDoubleQuoted => {
                    todo!("DOCTYPESystemIdentifierDoubleQuoted state - see STEP 3")
                }
                TokenizerState::DOCTYPESystemIdentifierSingleQuoted => {
                    todo!("DOCTYPESystemIdentifierSingleQuoted state - see STEP 3")
                }
                TokenizerState::AfterDOCTYPESystemIdentifier => {
                    todo!("AfterDOCTYPESystemIdentifier state - see STEP 3")
                }

                // STEP 4: BogusDOCTYPE - error recovery for malformed DOCTYPEs
                //   [§ 13.2.5.68](https://html.spec.whatwg.org/multipage/parsing.html#bogus-doctype-state)
                TokenizerState::BogusDOCTYPE => {
                    todo!("BogusDOCTYPE state - see STEP 4")
                }

                // ===== CDATA SECTION STATES =====
                // [§ 13.2.5.69-71](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-state)
                //
                // CDATA sections are only valid in foreign content (SVG/MathML):
                //   <![CDATA[ ... ]]>
                //
                // TODO: Implement CDATA parsing:
                //
                // STEP 5: CDATASection - consume characters until "]]>"
                //   [§ 13.2.5.69](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-state)
                TokenizerState::CDATASection => {
                    todo!("CDATASection state - see STEP 5")
                }
                // STEP 6: CDATASectionBracket - saw first ']'
                //   [§ 13.2.5.70](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-bracket-state)
                TokenizerState::CDATASectionBracket => {
                    todo!("CDATASectionBracket state - see STEP 6")
                }
                // STEP 7: CDATASectionEnd - saw "]]", looking for '>'
                //   [§ 13.2.5.71](https://html.spec.whatwg.org/multipage/parsing.html#cdata-section-end-state)
                TokenizerState::CDATASectionEnd => {
                    todo!("CDATASectionEnd state - see STEP 7")
                }
                // ===== CHARACTER REFERENCE STATES =====
                // [§ 13.2.5.72-80](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
                //
                // Character references encode special characters: &amp; &#60; &#x3C;
                //
                // Named references (implemented):
                TokenizerState::CharacterReference => self.handle_character_reference_state(),
                TokenizerState::NamedCharacterReference => {
                    self.handle_named_character_reference_state()
                }
                TokenizerState::AmbiguousAmpersand => self.handle_ambiguous_ampersand_state(),

                // TODO: Implement numeric character references:
                //
                // STEP 8: NumericCharacterReference - saw "&#", determine hex or decimal
                //   [§ 13.2.5.75](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-state)
                //   "Consume the next input character:"
                //   - "X" or "x": switch to HexadecimalCharacterReferenceStart
                //   - Anything else: reconsume in DecimalCharacterReferenceStart
                TokenizerState::NumericCharacterReference => {
                    self.handle_numeric_character_reference_state()
                }

                // STEP 9: Hexadecimal start - expect hex digits after "&#x"
                //   [§ 13.2.5.76](https://html.spec.whatwg.org/multipage/parsing.html#hexadecimal-character-reference-start-state)
                TokenizerState::HexadecimalCharacterReferenceStart => {
                    todo!("HexadecimalCharacterReferenceStart state - see STEP 9")
                }

                // STEP 10: Decimal start - expect digits after "&#"
                //   [§ 13.2.5.77](https://html.spec.whatwg.org/multipage/parsing.html#decimal-character-reference-start-state)
                TokenizerState::DecimalCharacterReferenceStart => {
                    todo!("DecimalCharacterReferenceStart state - see STEP 10")
                }

                // STEP 11: Hexadecimal digits - accumulate hex value
                //   [§ 13.2.5.78](https://html.spec.whatwg.org/multipage/parsing.html#hexadecimal-character-reference-state)
                //   Multiply accumulated value by 16, add digit value
                TokenizerState::HexadecimalCharacterReference => {
                    todo!("HexadecimalCharacterReference state - see STEP 11")
                }

                // STEP 12: Decimal digits - accumulate decimal value
                //   [§ 13.2.5.79](https://html.spec.whatwg.org/multipage/parsing.html#decimal-character-reference-state)
                //   Multiply accumulated value by 10, add digit value
                TokenizerState::DecimalCharacterReference => {
                    todo!("DecimalCharacterReference state - see STEP 12")
                }

                // STEP 13: Numeric end - convert code point, emit character
                //   [§ 13.2.5.80](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state)
                //   - Check for null (0x00), out of range (>0x10FFFF), surrogate, noncharacter
                //   - Apply replacement table for C1 controls (0x80-0x9F)
                //   - Emit the character token
                TokenizerState::NumericCharacterReferenceEnd => {
                    todo!("NumericCharacterReferenceEnd state - see STEP 13")
                }
            }
        }
    }
}
