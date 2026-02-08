//! Helper functions for the HTML tokenizer.
//!
//! [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
//!
//! This module contains utility functions used throughout the tokenizer:
//! - State transitions ("Switch to", "Reconsume in")
//! - Input/character handling ("Consume the next input character")
//! - Token emission ("Emit the current token")
//! - RCDATA/RAWTEXT helpers for raw text elements
//! - Attribute helpers for duplicate detection

use koala_common::warning::warn_once;

use super::core::{HTMLTokenizer, TokenizerState};
use super::token::Token;

// =============================================================================
// State Transition Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// "Switch to the X state"
    ///
    /// Transitions to a new state. The next character will be consumed on the
    /// next iteration of the main loop.
    pub(super) const fn switch_to(&mut self, new_state: TokenizerState) {
        self.state = new_state;
    }

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// "Reconsume in the X state"
    ///
    /// Transitions to a new state without consuming the current character.
    /// The same character will be processed again in the new state.
    pub(super) const fn reconsume_in(&mut self, new_state: TokenizerState) {
        self.reconsume = true;
        self.state = new_state;
    }
}

// =============================================================================
// Input/Character Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// "Consume the next input character"
    ///
    /// Returns the character at the current position and advances the position.
    /// Returns None if we've reached the end of input.
    pub(super) fn consume(&mut self) -> Option<char> {
        if let Some(c) = self.input[self.current_pos..].chars().next() {
            self.current_pos += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// Peek at a codepoint at the given offset from the current position without
    /// consuming it. Used for lookahead operations like "the next few characters are".
    #[must_use]
    pub fn peek_codepoint(&self, offset: usize) -> Option<char> {
        let slice = &self.input[self.current_pos..];
        slice.chars().nth(offset)
    }

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// "If the next few characters are..."
    ///
    /// Check if the next few characters match the target string exactly.
    #[must_use]
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

    /// [§ 13.2.5.42 Markup declaration open state](https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state)
    ///
    /// "ASCII case-insensitive match for the word 'DOCTYPE'"
    ///
    /// Check if the next few characters match the target string using
    /// ASCII case-insensitive comparison.
    #[must_use]
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

    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    ///
    /// Consume the given string from the input.
    /// Caller must have already verified the characters are present.
    pub const fn consume_string(&mut self, target: &str) {
        // Advance by the number of bytes in the target string.
        // This is safe for ASCII strings (like "DOCTYPE", "--", "[CDATA[").
        self.current_pos += target.len();
    }

    /// [§ 12.1.4 ASCII whitespace](https://infra.spec.whatwg.org/#ascii-whitespace)
    ///
    /// "ASCII whitespace is U+0009 TAB, U+000A LF, U+000C FF, U+000D CR,
    /// or U+0020 SPACE."
    ///
    /// NOTE: HTML tokenizer uses a subset excluding CR (which is normalized earlier).
    pub(super) const fn is_whitespace_char(input_char: char) -> bool {
        // "U+0009 CHARACTER TABULATION (tab)"
        // "U+000A LINE FEED (LF)"
        // "U+000C FORM FEED (FF)"
        // "U+0020 SPACE"
        matches!(input_char, ' ' | '\t' | '\n' | '\x0C')
    }
}

// =============================================================================
// Token Emission Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
    // "Emit the current token" - adds the token to the output stream.
    pub fn emit_token(&mut self) {
        if let Some(token) = self.current_token.take() {
            // Track the last start tag name for RCDATA/RAWTEXT end tag detection
            if let Token::StartTag { ref name, .. } = token {
                self.last_start_tag_name = Some(name.clone());

                // [§ 13.2.6.4.4 The "in head" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
                // [§ 13.2.6.4.7 The "in body" insertion mode](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
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
                    // "A start tag whose tag name is \"script\""
                    // "Follow the generic script element parsing algorithm."
                    // [§ 13.2.6.4](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead)
                    // "Switch the tokenizer to the ScriptData state."
                    "script" => {
                        self.token_stream.push(token);
                        self.switch_to(TokenizerState::ScriptData);
                        return;
                    }
                    _ => {}
                }
            }
            self.token_stream.push(token);
        }
    }

    /// "Emit the current input character as a character token."
    ///
    /// Emits a character token directly without going through `current_token`.
    pub fn emit_character_token(&mut self, c: char) {
        let token = Token::new_character(c);
        self.token_stream.push(token);
    }

    /// "Emit an end-of-file token."
    pub fn emit_eof_token(&mut self) {
        let token = Token::new_eof();
        self.token_stream.push(token);
    }
}

// =============================================================================
// RCDATA/RAWTEXT Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    /// [§ 13.2.5.17 Script data end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-name-state)
    ///
    /// "An appropriate end tag token is an end tag token whose tag name matches
    /// the tag name of the last start tag to have been emitted from this
    /// tokenizer, if any."
    ///
    /// Used to determine if `</title>` should close the current `<title>` element.
    pub(super) fn is_appropriate_end_tag_token(&self) -> bool {
        if let (Some(last_start_tag), Some(Token::EndTag { name, .. })) =
            (&self.last_start_tag_name, &self.current_token)
        {
            return name == last_start_tag;
        }
        false
    }

    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    ///
    /// "Anything else":
    /// "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character
    /// token, and a character token for each of the characters in the temporary
    /// buffer... Reconsume in the RCDATA state."
    pub(super) fn emit_rcdata_end_tag_name_anything_else(&mut self) {
        // STEP 1: "Emit a U+003C LESS-THAN SIGN character token"
        self.emit_character_token('<');
        // STEP 2: "Emit a U+002F SOLIDUS character token"
        self.emit_character_token('/');
        // STEP 3: "Emit a character token for each of the characters in the temporary buffer"
        let buffer = self.temporary_buffer.clone();
        for c in buffer.chars() {
            self.emit_character_token(c);
        }
        // STEP 4: Discard the current end tag token
        self.current_token = None;
        // STEP 5: "Reconsume in the RCDATA state"
        self.reconsume_in(TokenizerState::RCDATA);
    }

    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    ///
    /// "Anything else":
    /// "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character
    /// token, and a character token for each of the characters in the temporary
    /// buffer... Reconsume in the RAWTEXT state."
    pub(super) fn emit_rawtext_end_tag_name_anything_else(&mut self) {
        // STEP 1: "Emit a U+003C LESS-THAN SIGN character token"
        self.emit_character_token('<');
        // STEP 2: "Emit a U+002F SOLIDUS character token"
        self.emit_character_token('/');
        // STEP 3: "Emit a character token for each of the characters in the temporary buffer"
        let buffer = self.temporary_buffer.clone();
        for c in buffer.chars() {
            self.emit_character_token(c);
        }
        // STEP 4: Discard the current end tag token
        self.current_token = None;
        // STEP 5: "Reconsume in the RAWTEXT state"
        self.reconsume_in(TokenizerState::RAWTEXT);
    }

    /// [§ 13.2.5.17 Script data end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-name-state)
    ///
    /// "Anything else":
    /// "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character
    /// token, and a character token for each of the characters in the temporary
    /// buffer... Reconsume in the script data state."
    pub(super) fn emit_script_data_end_tag_name_anything_else(&mut self) {
        // STEP 1: "Emit a U+003C LESS-THAN SIGN character token"
        self.emit_character_token('<');
        // STEP 2: "Emit a U+002F SOLIDUS character token"
        self.emit_character_token('/');
        // STEP 3: "Emit a character token for each of the characters in the temporary buffer"
        let buffer = self.temporary_buffer.clone();
        for c in buffer.chars() {
            self.emit_character_token(c);
        }
        // STEP 4: Discard the current end tag token
        self.current_token = None;
        // STEP 5: "Reconsume in the script data state"
        self.reconsume_in(TokenizerState::ScriptData);
    }

    /// [§ 13.2.5.25 Script data escaped end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-escaped-end-tag-name-state)
    ///
    /// "Anything else":
    /// "Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character
    /// token, and a character token for each of the characters in the temporary
    /// buffer... Reconsume in the script data escaped state."
    pub(super) fn emit_escaped_end_tag_name_anything_else(&mut self) {
        self.emit_character_token('<');
        self.emit_character_token('/');
        let buffer = self.temporary_buffer.clone();
        for c in buffer.chars() {
            self.emit_character_token(c);
        }
        self.current_token = None;
        self.reconsume_in(TokenizerState::ScriptDataEscaped);
    }
}

// =============================================================================
// Attribute Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5.33 Before attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state)
    /// [§ 13.2.5.34 Attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#attribute-name-state)
    ///
    /// "When the user agent leaves the attribute name state... if there is
    /// already an attribute on the token with the exact same name, then this
    /// is a duplicate-attribute parse error and the new attribute must be
    /// removed from the token."
    ///
    /// Helper to check for duplicate attributes and handle the parse error.
    pub(super) fn check_duplicate_attribute(&mut self) {
        // STEP 1: Check if the current attribute name already exists on the token.
        // This avoids borrow checker issues by not holding a mutable borrow
        // while calling log_parse_error.
        let is_duplicate = self
            .current_token
            .as_ref()
            .is_some_and(Token::current_attribute_name_is_duplicate);

        if is_duplicate {
            // STEP 2: "This is a duplicate-attribute parse error"
            self.log_parse_error();

            // STEP 3: "The new attribute must be removed from the token"
            if let Some(ref mut token) = self.current_token {
                token.remove_current_attribute();
            }
        }
    }
}

// =============================================================================
// Error Handling
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.2 Parse errors](https://html.spec.whatwg.org/multipage/parsing.html#parse-errors)
    ///
    /// Logs a parse error using the koala-common warning system.
    /// Parse errors in HTML are not fatal - the parser recovers and continues.
    pub(super) fn log_parse_error(&self) {
        let pos = self.current_pos;
        warn_once("HTML Tokenizer", &format!("parse error at position {pos}"));
    }
}
