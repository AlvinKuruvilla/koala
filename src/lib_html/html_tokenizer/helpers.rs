//! Helper functions for the HTML tokenizer.
//!
//! This module contains utility functions used throughout the tokenizer:
//! - State transitions
//! - Input/character handling
//! - Token emission
//! - RCDATA/RAWTEXT helpers
//! - Attribute helpers

use super::token::Token;
use super::tokenizer::{HTMLTokenizer, TokenizerState};

// =============================================================================
// State Transition Helpers
// =============================================================================

impl HTMLTokenizer {
    // "Switch to the X state"
    // Transitions to a new state. The next character will be consumed on the next
    // iteration of the main loop.
    pub(super) fn switch_to(&mut self, new_state: TokenizerState) {
        self.state = new_state;
    }

    // "Reconsume in the X state"
    // Transitions to a new state without consuming the current character.
    // The same character will be processed again in the new state.
    pub(super) fn reconsume_in(&mut self, new_state: TokenizerState) {
        self.reconsume = true;
        self.state = new_state;
    }
}

// =============================================================================
// Input/Character Helpers
// =============================================================================

impl HTMLTokenizer {
    // "Consume the next input character"
    // Returns the character at the current position and advances the position.
    pub(super) fn consume(&mut self) -> Option<char> {
        if let Some(c) = self.input[self.current_pos..].chars().next() {
            self.current_pos += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    // Use peek to view the next codepoint at a given offset without advancing
    pub fn peek_codepoint(&self, offset: usize) -> Option<char> {
        let slice = &self.input[self.current_pos..];
        slice.chars().nth(offset)
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

    pub(super) fn is_whitespace_char(input_char: char) -> bool {
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
}

// =============================================================================
// RCDATA/RAWTEXT Helpers
// =============================================================================

impl HTMLTokenizer {
    /// [§ 13.2.5.11 RCDATA end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rcdata-end-tag-name-state)
    /// [§ 13.2.5.14 RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state)
    ///
    // "An appropriate end tag token is an end tag token whose tag name matches the tag name
    // of the last start tag to have been emitted from this tokenizer, if any."
    pub(super) fn is_appropriate_end_tag_token(&self) -> bool {
        if let (Some(ref last_start_tag), Some(ref current_token)) =
            (&self.last_start_tag_name, &self.current_token)
        {
            if let Token::EndTag { name, .. } = current_token {
                return name == last_start_tag;
            }
        }
        false
    }

    /// Helper for RCDATA end tag name state "anything else" branch.
    pub(super) fn emit_rcdata_end_tag_name_anything_else(&mut self) {
        self.emit_character_token('<');
        self.emit_character_token('/');
        for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
            self.emit_character_token(c);
        }
        self.current_token = None;
        self.reconsume_in(TokenizerState::RCDATA);
    }

    /// Helper for RAWTEXT end tag name state "anything else" branch.
    pub(super) fn emit_rawtext_end_tag_name_anything_else(&mut self) {
        self.emit_character_token('<');
        self.emit_character_token('/');
        for c in self.temporary_buffer.chars().collect::<Vec<_>>() {
            self.emit_character_token(c);
        }
        self.current_token = None;
        self.reconsume_in(TokenizerState::RAWTEXT);
    }
}

// =============================================================================
// Attribute Helpers
// =============================================================================

impl HTMLTokenizer {
    /// Helper to check for duplicate attributes and handle the parse error.
    pub(super) fn check_duplicate_attribute(&mut self) {
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
}

// =============================================================================
// Error Handling
// =============================================================================

impl HTMLTokenizer {
    pub(super) fn log_parse_error(&self) {
        // Debug output disabled
        // println!("Parse error at position {}", self.current_pos);
    }
}
