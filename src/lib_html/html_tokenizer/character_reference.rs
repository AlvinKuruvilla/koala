//! Character reference helpers for the HTML tokenizer.
//!
//! [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)

use super::tokenizer::{HTMLTokenizer, TokenizerState};

impl HTMLTokenizer {
    /// [§ 13.2.5.72 Character reference state](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state)
    /// Returns true if the return state is an attribute value state.
    /// Per spec: "consumed as part of an attribute"
    pub(super) fn is_consumed_as_part_of_attribute(&self) -> bool {
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
    pub(super) fn flush_code_points_consumed_as_character_reference(&mut self) {
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
}
