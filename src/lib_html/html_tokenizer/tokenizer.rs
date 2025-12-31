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
    // "Reconsume in the X state" sets this flag.
    reconsume: bool,

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    /// "The last start tag token emitted is used as part of the tree construction stage
    /// and in the RCDATA, RAWTEXT, and script data states."
    last_start_tag_name: Option<String>,

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#temporary-buffer)
    /// "The temporary buffer is used to temporarily store characters during certain
    /// tokenization operations, particularly for end tag detection in RCDATA/RAWTEXT states."
    temporary_buffer: String,
}
impl HTMLTokenizer {
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

    // "Switch to the X state"
    // Transitions to a new state. The next character will be consumed on the next
    // iteration of the main loop.
    fn switch_to(&mut self, new_state: TokenizerState) {
        self.state = new_state;
    }

    // "Reconsume in the X state"
    // Transitions to a new state without consuming the current character.
    // The same character will be processed again in the new state.
    fn reconsume_in(&mut self, new_state: TokenizerState) {
        self.reconsume = true;
        self.state = new_state;
    }

    fn log_parse_error(&self) {
        // Debug output disabled
        // println!("Parse error at position {}", self.current_pos);
    }
    fn is_whitespace_char(input_char: char) -> bool {
        matches!(input_char, ' ' | '\t' | '\n' | '\x0C')
    }

    /// [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
    /// Returns true if the return state is an attribute value state.
    /// Per spec: "consumed as part of an attribute"
    fn is_consumed_as_part_of_attribute(&self) -> bool {
        matches!(
            self.return_state,
            Some(TokenizerState::AttributeValueDoubleQuoted)
                | Some(TokenizerState::AttributeValueSingleQuoted)
                | Some(TokenizerState::AttributeValueUnquoted)
        )
    }

    /// [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
    /// "Flush code points consumed as a character reference"
    /// Per spec: "If the character reference was consumed as part of an attribute,
    /// then append each character to the current attribute's value. Otherwise,
    /// emit each character as a character token."
    fn flush_code_points_consumed_as_character_reference(&mut self) {
        if self.is_consumed_as_part_of_attribute() {
            for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
                if let Some(ref mut token) = self.current_token {
                    token.append_to_current_attribute_value(c);
                }
            }
        } else {
            for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
                self.emit_character_token(c);
            }
        }
    }

    // [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    // "Emit the current token" - adds the token to the output stream.
    pub fn emit_token(&mut self) {
        if let Some(token) = self.current_token.take() {
            // Track the last start tag name for RCDATA/RAWTEXT end tag detection
            if let Token::StartTag { ref name, .. } = token {
                self.last_start_tag_name = Some(name.clone());

                // [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
                // [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
                //
                // NOTE: Per spec, the parser should switch the tokenizer state. Since we run
                // the tokenizer before the parser, we detect special elements here and switch
                // states accordingly.
                //
                // RCDATA elements: "title", "textarea"
                // RAWTEXT elements: "style", "xmp", "iframe", "noembed", "noframes"
                // Script data: "script" (more complex, not yet implemented)
                match name.as_str() {
                    // "A start tag whose tag name is "title""
                    // "Follow the generic RCDATA element parsing algorithm."
                    // [§ 13.2.6.2](https://html.spec.whatwg.org/multipage/parsing.html#generic-rcdata-element-parsing-algorithm)
                    // "Switch the tokenizer to the RCDATA state."
                    "title" | "textarea" => {
                        self.token_stream.push(token);
                        self.switch_to(TokenizerState::RCDATA);
                        return;
                    }
                    // "A start tag whose tag name is one of: "style", "xmp", "iframe", "noembed", "noframes""
                    // "Follow the generic raw text element parsing algorithm."
                    // [§ 13.2.6.3](https://html.spec.whatwg.org/multipage/parsing.html#generic-raw-text-element-parsing-algorithm)
                    // "Switch the tokenizer to the RAWTEXT state."
                    "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                        self.token_stream.push(token);
                        self.switch_to(TokenizerState::RAWTEXT);
                        return;
                    }
                    // NOTE: "script" requires ScriptData state which is more complex.
                    // Left as todo!() for now.
                    _ => {}
                }
            }
            self.token_stream.push(token);
        }
    }

    // "Emit the current input character as a character token."
    // Emits a character token directly without going through current_token.
    pub fn emit_character_token(&mut self, c: char) {
        let token = Token::new_character(c);
        self.token_stream.push(token);
    }

    // "Emit an end-of-file token."
    pub fn emit_eof_token(&mut self) {
        let token = Token::new_eof();
        self.token_stream.push(token);
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

    /// Helper for RCDATA end tag name state "anything else" branch.
    fn emit_rcdata_end_tag_name_anything_else(&mut self) {
        self.emit_character_token('<');
        self.emit_character_token('/');
        for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
            self.emit_character_token(c);
        }
        self.current_token = None;
        self.reconsume_in(TokenizerState::RCDATA);
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

    /// Helper for RAWTEXT end tag name state "anything else" branch.
    fn emit_rawtext_end_tag_name_anything_else(&mut self) {
        self.emit_character_token('<');
        self.emit_character_token('/');
        for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
            self.emit_character_token(c);
        }
        self.current_token = None;
        self.reconsume_in(TokenizerState::RAWTEXT);
    }

    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    ///
    /// "An appropriate end tag token is an end tag token whose tag name matches the tag name
    /// of the last start tag to have been emitted from this tokenizer, if any."
    fn is_appropriate_end_tag_token(&self) -> bool {
        if let (Some(ref last_start_tag), Some(ref current_token)) =
            (&self.last_start_tag_name, &self.current_token)
        {
            if let Token::EndTag { name, .. } = current_token {
                return name == last_start_tag;
            }
        }
        false
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

    // "Consume the next input character"
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
    /// Used by [§ 13.2.5.42 Markup declaration open state](https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state)
    /// for "ASCII case-insensitive match".
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
                TokenizerState::CharacterReference => self.handle_character_reference_state(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to tokenize a string and return the tokens (excluding printing)
    fn tokenize(input: &str) -> Vec<Token> {
        let mut tokenizer = HTMLTokenizer::new(input.to_string());
        tokenizer.run();
        tokenizer.into_tokens()
    }

    #[test]
    fn test_plain_text() {
        let tokens = tokenize("Hello");
        assert_eq!(tokens.len(), 6); // 5 chars + EOF
        assert!(matches!(tokens[0], Token::Character { data: 'H' }));
        assert!(matches!(tokens[4], Token::Character { data: 'o' }));
        assert!(matches!(tokens[5], Token::EndOfFile));
    }

    #[test]
    fn test_doctype() {
        let tokens = tokenize("<!DOCTYPE html>");
        assert_eq!(tokens.len(), 2); // DOCTYPE + EOF
        match &tokens[0] {
            Token::Doctype { name, force_quirks, .. } => {
                assert_eq!(name.as_deref(), Some("html"));
                assert!(!force_quirks);
            }
            _ => panic!("Expected DOCTYPE token"),
        }
    }

    #[test]
    fn test_start_tag() {
        let tokens = tokenize("<div>");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            Token::StartTag { name, self_closing, attributes } => {
                assert_eq!(name, "div");
                assert!(!self_closing);
                assert!(attributes.is_empty());
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_end_tag() {
        let tokens = tokenize("</div>");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            Token::EndTag { name, .. } => {
                assert_eq!(name, "div");
            }
            _ => panic!("Expected EndTag token"),
        }
    }

    #[test]
    fn test_self_closing_tag() {
        let tokens = tokenize("<br/>");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            Token::StartTag { name, self_closing, .. } => {
                assert_eq!(name, "br");
                assert!(self_closing);
            }
            _ => panic!("Expected self-closing StartTag token"),
        }
    }

    #[test]
    fn test_comment() {
        let tokens = tokenize("<!-- hello -->");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            Token::Comment { data } => {
                assert_eq!(data, " hello ");
            }
            _ => panic!("Expected Comment token"),
        }
    }

    #[test]
    fn test_attribute_double_quoted() {
        let tokens = tokenize(r#"<div class="foo">"#);
        match &tokens[0] {
            Token::StartTag { name, attributes, .. } => {
                assert_eq!(name, "div");
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "class");
                assert_eq!(attributes[0].value, "foo");
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_attribute_single_quoted() {
        let tokens = tokenize("<div class='bar'>");
        match &tokens[0] {
            Token::StartTag { name, attributes, .. } => {
                assert_eq!(name, "div");
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "class");
                assert_eq!(attributes[0].value, "bar");
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_attribute_unquoted() {
        let tokens = tokenize("<div class=baz>");
        match &tokens[0] {
            Token::StartTag { name, attributes, .. } => {
                assert_eq!(name, "div");
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "class");
                assert_eq!(attributes[0].value, "baz");
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_boolean_attribute() {
        let tokens = tokenize("<input disabled>");
        match &tokens[0] {
            Token::StartTag { name, attributes, .. } => {
                assert_eq!(name, "input");
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "disabled");
                assert_eq!(attributes[0].value, "");
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_multiple_attributes() {
        let tokens = tokenize(r#"<input type="text" id="name" disabled>"#);
        match &tokens[0] {
            Token::StartTag { name, attributes, .. } => {
                assert_eq!(name, "input");
                assert_eq!(attributes.len(), 3);
                assert_eq!(attributes[0].name, "type");
                assert_eq!(attributes[0].value, "text");
                assert_eq!(attributes[1].name, "id");
                assert_eq!(attributes[1].value, "name");
                assert_eq!(attributes[2].name, "disabled");
                assert_eq!(attributes[2].value, "");
            }
            _ => panic!("Expected StartTag token"),
        }
    }

    #[test]
    fn test_tag_with_text_content() {
        let tokens = tokenize("<p>Hi</p>");
        assert_eq!(tokens.len(), 5); // <p>, H, i, </p>, EOF
        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "p"));
        assert!(matches!(tokens[1], Token::Character { data: 'H' }));
        assert!(matches!(tokens[2], Token::Character { data: 'i' }));
        assert!(matches!(&tokens[3], Token::EndTag { name, .. } if name == "p"));
        assert!(matches!(tokens[4], Token::EndOfFile));
    }

    #[test]
    fn test_simple_html_document() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>Hello</body>
</html>"#;
        let tokens = tokenize(html);

        // Should have DOCTYPE as first token
        assert!(matches!(&tokens[0], Token::Doctype { name: Some(n), .. } if n == "html"));

        // Should end with EOF
        assert!(matches!(tokens.last(), Some(Token::EndOfFile)));

        // Count tag tokens
        let start_tags: Vec<_> = tokens.iter().filter(|t| matches!(t, Token::StartTag { .. })).collect();
        let end_tags: Vec<_> = tokens.iter().filter(|t| matches!(t, Token::EndTag { .. })).collect();

        assert_eq!(start_tags.len(), 4); // html, head, title, body
        assert_eq!(end_tags.len(), 4);   // /title, /head, /body, /html
    }

    // ========== Raw text element (RCDATA/RAWTEXT) tests ==========

    #[test]
    fn test_style_element_rawtext() {
        // Style content should be treated as raw text, not parsed as tags
        let tokens = tokenize("<style>body { color: red; }</style>");

        // Should have: <style>, characters for content, </style>, EOF
        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "style"));

        // Collect the character content
        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(content, "body { color: red; }");

        // Last tokens should be </style> and EOF
        assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "style"));
        assert!(matches!(tokens.last(), Some(Token::EndOfFile)));
    }

    #[test]
    fn test_title_element_rcdata() {
        // Title content should be treated as RCDATA (raw text, but character references are parsed)
        let tokens = tokenize("<title>My Page</title>");

        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "title"));

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(content, "My Page");
        assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "title"));
    }

    #[test]
    fn test_style_with_fake_tags() {
        // Tags inside style should NOT be parsed as tags
        let tokens = tokenize("<style><div>not a tag</div></style>");

        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "style"));

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        // The <div> and </div> should appear as literal text, not as tags
        assert_eq!(content, "<div>not a tag</div>");
        assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "style"));
    }

    #[test]
    fn test_title_with_less_than() {
        // Less-than signs in title should be emitted as characters
        let tokens = tokenize("<title>a < b</title>");

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(content, "a < b");
    }

    #[test]
    fn test_style_with_wrong_end_tag() {
        // </notastyle> inside style should NOT close the style element
        let tokens = tokenize("<style>a</notastyle>b</style>");

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        // The </notastyle> should appear as literal text
        assert_eq!(content, "a</notastyle>b");
    }

    #[test]
    fn test_textarea_element_rcdata() {
        let tokens = tokenize("<textarea><b>bold?</b></textarea>");

        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "textarea"));

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        // Content should be literal text, not parsed tags
        assert_eq!(content, "<b>bold?</b>");
        assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "textarea"));
    }

    #[test]
    fn test_xmp_element_rawtext() {
        let tokens = tokenize("<xmp><html>is text</html></xmp>");

        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "xmp"));

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(content, "<html>is text</html>");
    }

    #[test]
    fn test_iframe_element_rawtext() {
        let tokens = tokenize("<iframe>some content</iframe>");

        assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "iframe"));

        let content: String = tokens[1..tokens.len() - 2]
            .iter()
            .filter_map(|t| {
                if let Token::Character { data } = t {
                    Some(*data)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(content, "some content");
    }

    #[test]
    fn test_character_reference_bare_ampersand() {
        // [§ 13.2.5.72 Character reference state]
        // Bare ampersand followed by non-alphanumeric should flush as literal '&'
        let tokens = tokenize("a & b");
        // Should be: 'a', ' ', '&', ' ', 'b', EOF
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], Token::Character { data: 'a' }));
        assert!(matches!(tokens[1], Token::Character { data: ' ' }));
        assert!(matches!(tokens[2], Token::Character { data: '&' }));
        assert!(matches!(tokens[3], Token::Character { data: ' ' }));
        assert!(matches!(tokens[4], Token::Character { data: 'b' }));
        assert!(matches!(tokens[5], Token::EndOfFile));
    }
}
