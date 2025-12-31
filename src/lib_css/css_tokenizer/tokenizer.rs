use super::token::{CSSToken, HashType, NumericType};

/// [§ 4.3 Tokenizer Algorithms](https://www.w3.org/TR/css-syntax-3/#tokenizer-algorithms)
///
/// CSS tokenizer following the CSS Syntax Module Level 3 specification.
pub struct CSSTokenizer {
    /// The input string being tokenized
    input: Vec<char>,
    /// Current position in the input
    position: usize,
    /// Collected tokens
    tokens: Vec<CSSToken>,
}

impl CSSTokenizer {
    /// Create a new CSS tokenizer with the given input.
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into().chars().collect(),
            position: 0,
            tokens: Vec::new(),
        }
    }

    /// [§ 4.3.1 Consume a token](https://www.w3.org/TR/css-syntax-3/#consume-token)
    ///
    /// "This section describes how to consume a token from a stream of code points.
    /// It will return a single token of any type."
    pub fn run(&mut self) {
        loop {
            let token = self.consume_token();
            let is_eof = token.is_eof();
            self.tokens.push(token);
            if is_eof {
                break;
            }
        }
    }

    /// Return the collected tokens.
    pub fn into_tokens(self) -> Vec<CSSToken> {
        self.tokens
    }

    /// Return a reference to the collected tokens.
    pub fn tokens(&self) -> &[CSSToken] {
        &self.tokens
    }

    /// [§ 4.3.1 Consume a token](https://www.w3.org/TR/css-syntax-3/#consume-token)
    fn consume_token(&mut self) -> CSSToken {
        // "Consume comments."
        self.consume_comments();

        // "Consume the next input code point."
        let c = match self.consume() {
            Some(c) => c,
            None => return CSSToken::EOF,
        };

        match c {
            // "whitespace"
            // "Consume as much whitespace as possible. Return a <whitespace-token>."
            c if is_whitespace(c) => {
                self.consume_whitespace();
                CSSToken::Whitespace
            }

            // "U+0022 QUOTATION MARK (")"
            // "Consume a string token and return it."
            '"' => self.consume_string_token('"'),

            // "U+0023 NUMBER SIGN (#)"
            '#' => {
                // "If the next input code point is an ident code point or the next
                // two input code points are a valid escape..."
                if self.peek().map(is_ident_code_point).unwrap_or(false)
                    || self.is_valid_escape(self.peek(), self.peek_at(1))
                {
                    // "Create a <hash-token>."
                    // "If the next 3 input code points would start an ident sequence,
                    // set the <hash-token>'s type flag to 'id'."
                    let hash_type = if self.would_start_ident_sequence() {
                        HashType::Id
                    } else {
                        HashType::Unrestricted
                    };

                    // "Consume an ident sequence, and set the <hash-token>'s value
                    // to the returned string."
                    let value = self.consume_ident_sequence();

                    CSSToken::Hash { value, hash_type }
                } else {
                    // "Otherwise, return a <delim-token> with its value set to the
                    // current input code point."
                    CSSToken::Delim('#')
                }
            }

            // "U+0027 APOSTROPHE (')"
            // "Consume a string token and return it."
            '\'' => self.consume_string_token('\''),

            // "U+0028 LEFT PARENTHESIS (()"
            // "Return a <(-token>."
            '(' => CSSToken::LeftParen,

            // "U+0029 RIGHT PARENTHESIS ())"
            // "Return a <)-token>."
            ')' => CSSToken::RightParen,

            // "U+002B PLUS SIGN (+)"
            '+' => {
                // "If the input stream starts with a number..."
                if self.would_start_number() {
                    // "Reconsume the current input code point."
                    self.reconsume();
                    // "Consume a numeric token and return it."
                    self.consume_numeric_token()
                } else {
                    CSSToken::Delim('+')
                }
            }

            // "U+002C COMMA (,)"
            // "Return a <comma-token>."
            ',' => CSSToken::Comma,

            // "U+002D HYPHEN-MINUS (-)"
            '-' => {
                // "If the input stream starts with a number..."
                if self.would_start_number() {
                    self.reconsume();
                    self.consume_numeric_token()
                }
                // "Otherwise, if the next 2 input code points are U+002D U+003E (->)..."
                else if self.peek() == Some('-') && self.peek_at(1) == Some('>') {
                    self.consume(); // -
                    self.consume(); // >
                    CSSToken::CDC
                }
                // "Otherwise, if the input stream starts with an ident sequence..."
                else if self.would_start_ident_sequence_with(Some('-')) {
                    self.reconsume();
                    self.consume_ident_like_token()
                } else {
                    CSSToken::Delim('-')
                }
            }

            // "U+002E FULL STOP (.)"
            '.' => {
                // "If the input stream starts with a number..."
                if self.would_start_number() {
                    self.reconsume();
                    self.consume_numeric_token()
                } else {
                    CSSToken::Delim('.')
                }
            }

            // "U+003A COLON (:)"
            // "Return a <colon-token>."
            ':' => CSSToken::Colon,

            // "U+003B SEMICOLON (;)"
            // "Return a <semicolon-token>."
            ';' => CSSToken::Semicolon,

            // "U+003C LESS-THAN SIGN (<)"
            '<' => {
                // "If the next 3 input code points are U+0021 U+002D U+002D (!--)..."
                if self.peek() == Some('!')
                    && self.peek_at(1) == Some('-')
                    && self.peek_at(2) == Some('-')
                {
                    self.consume(); // !
                    self.consume(); // -
                    self.consume(); // -
                    CSSToken::CDO
                } else {
                    CSSToken::Delim('<')
                }
            }

            // "U+0040 COMMERCIAL AT (@)"
            '@' => {
                // "If the next 3 input code points would start an ident sequence..."
                if self.would_start_ident_sequence() {
                    // "Consume an ident sequence, create an <at-keyword-token> with
                    // its value set to the returned value, and return it."
                    let value = self.consume_ident_sequence();
                    CSSToken::AtKeyword(value)
                } else {
                    CSSToken::Delim('@')
                }
            }

            // "U+005B LEFT SQUARE BRACKET ([)"
            // "Return a <[-token>."
            '[' => CSSToken::LeftBracket,

            // "U+005C REVERSE SOLIDUS (\)"
            '\\' => {
                // "If the input stream starts with a valid escape..."
                if self.is_valid_escape(Some('\\'), self.peek()) {
                    // "Reconsume the current input code point."
                    self.reconsume();
                    // "Consume an ident-like token and return it."
                    self.consume_ident_like_token()
                } else {
                    // "This is a parse error."
                    // "Return a <delim-token> with its value set to the current input code point."
                    CSSToken::Delim('\\')
                }
            }

            // "U+005D RIGHT SQUARE BRACKET (])"
            // "Return a <]-token>."
            ']' => CSSToken::RightBracket,

            // "U+007B LEFT CURLY BRACKET ({)"
            // "Return a <{-token>."
            '{' => CSSToken::LeftBrace,

            // "U+007D RIGHT CURLY BRACKET (})"
            // "Return a <}-token>."
            '}' => CSSToken::RightBrace,

            // "digit"
            // "Reconsume the current input code point. Consume a numeric token and return it."
            c if c.is_ascii_digit() => {
                self.reconsume();
                self.consume_numeric_token()
            }

            // "ident-start code point"
            // "Reconsume the current input code point. Consume an ident-like token and return it."
            c if is_ident_start_code_point(c) => {
                self.reconsume();
                self.consume_ident_like_token()
            }

            // "anything else"
            // "Return a <delim-token> with its value set to the current input code point."
            c => CSSToken::Delim(c),
        }
    }

    /// [§ 4.3.2 Consume comments](https://www.w3.org/TR/css-syntax-3/#consume-comment)
    ///
    /// "If the next two input code points are U+002F SOLIDUS (/) followed by
    /// U+002A ASTERISK (*), consume them and all following code points up to
    /// and including the first U+002A ASTERISK (*) followed by U+002F SOLIDUS (/),
    /// or up to an EOF code point."
    fn consume_comments(&mut self) {
        while self.peek() == Some('/') && self.peek_at(1) == Some('*') {
            self.consume(); // /
            self.consume(); // *

            loop {
                match self.consume() {
                    Some('*') if self.peek() == Some('/') => {
                        self.consume(); // /
                        break;
                    }
                    Some(_) => continue,
                    None => break, // EOF
                }
            }
        }
    }

    /// Consume whitespace characters.
    fn consume_whitespace(&mut self) {
        while self.peek().map(is_whitespace).unwrap_or(false) {
            self.consume();
        }
    }

    /// [§ 4.3.4 Consume a string token](https://www.w3.org/TR/css-syntax-3/#consume-string-token)
    fn consume_string_token(&mut self, ending_code_point: char) -> CSSToken {
        // "Initially create a <string-token> with its value set to the empty string."
        let mut value = String::new();

        loop {
            match self.consume() {
                // "ending code point"
                // "Return the <string-token>."
                Some(c) if c == ending_code_point => {
                    return CSSToken::String(value);
                }

                // "EOF"
                // "This is a parse error. Return the <string-token>."
                None => {
                    return CSSToken::String(value);
                }

                // "newline"
                // "This is a parse error. Reconsume the current input code point,
                // create a <bad-string-token>, and return it."
                Some('\n') => {
                    self.reconsume();
                    return CSSToken::BadString;
                }

                // "U+005C REVERSE SOLIDUS (\)"
                Some('\\') => {
                    match self.peek() {
                        // "If the next input code point is EOF, do nothing."
                        None => {}
                        // "Otherwise, if the next input code point is a newline,
                        // consume it."
                        Some('\n') => {
                            self.consume();
                        }
                        // "Otherwise, (the stream starts with a valid escape)
                        // consume an escaped code point and append the returned
                        // code point to the <string-token>'s value."
                        Some(_) => {
                            if let Some(c) = self.consume_escaped_code_point() {
                                value.push(c);
                            }
                        }
                    }
                }

                // "anything else"
                // "Append the current input code point to the <string-token>'s value."
                Some(c) => {
                    value.push(c);
                }
            }
        }
    }

    /// [§ 4.3.5 Consume a numeric token](https://www.w3.org/TR/css-syntax-3/#consume-numeric-token)
    fn consume_numeric_token(&mut self) -> CSSToken {
        // "Consume a number and let number be the result."
        let (value, int_value, numeric_type) = self.consume_number();

        // "If the next 3 input code points would start an ident sequence..."
        if self.would_start_ident_sequence() {
            // "Create a <dimension-token> with the same value and type flag as number,
            // and a unit set initially to the empty string."
            // "Consume an ident sequence. Set the <dimension-token>'s unit to the
            // returned value."
            let unit = self.consume_ident_sequence();
            CSSToken::Dimension {
                value,
                int_value,
                numeric_type,
                unit,
            }
        }
        // "Otherwise, if the next input code point is U+0025 PERCENTAGE SIGN (%)..."
        else if self.peek() == Some('%') {
            self.consume();
            CSSToken::Percentage {
                value,
                int_value,
                numeric_type,
            }
        }
        // "Otherwise, create a <number-token> with the same value and type flag as number,
        // and return it."
        else {
            CSSToken::Number {
                value,
                int_value,
                numeric_type,
            }
        }
    }

    /// [§ 4.3.6 Consume an ident-like token](https://www.w3.org/TR/css-syntax-3/#consume-ident-like-token)
    fn consume_ident_like_token(&mut self) -> CSSToken {
        // "Consume an ident sequence, and let string be the result."
        let string = self.consume_ident_sequence();

        // "If string's value is an ASCII case-insensitive match for 'url',
        // and the next input code point is U+0028 LEFT PARENTHESIS (()"
        if string.eq_ignore_ascii_case("url") && self.peek() == Some('(') {
            self.consume(); // (

            // Consume whitespace
            while self.peek().map(is_whitespace).unwrap_or(false) {
                self.consume();
            }

            // "If the next one or two input code points are U+0022 QUOTATION MARK,
            // U+0027 APOSTROPHE, or whitespace followed by U+0022 QUOTATION MARK or
            // U+0027 APOSTROPHE..."
            match self.peek() {
                Some('"') | Some('\'') => {
                    // "return a <function-token> with its value set to string"
                    CSSToken::Function(string)
                }
                _ => {
                    // "Otherwise, consume a url token, and return it."
                    self.consume_url_token()
                }
            }
        }
        // "Otherwise, if the next input code point is U+0028 LEFT PARENTHESIS (()"
        else if self.peek() == Some('(') {
            self.consume();
            // "Return a <function-token> with its value set to string."
            CSSToken::Function(string)
        }
        // "Otherwise, return an <ident-token> with its value set to string."
        else {
            CSSToken::Ident(string)
        }
    }

    /// [§ 4.3.7 Consume a url token](https://www.w3.org/TR/css-syntax-3/#consume-url-token)
    fn consume_url_token(&mut self) -> CSSToken {
        // "Initially create a <url-token> with its value set to the empty string."
        let mut value = String::new();

        // "Consume as much whitespace as possible."
        self.consume_whitespace();

        loop {
            match self.consume() {
                // "U+0029 RIGHT PARENTHESIS ())"
                // "Return the <url-token>."
                Some(')') => {
                    return CSSToken::Url(value);
                }

                // "EOF"
                // "This is a parse error. Return the <url-token>."
                None => {
                    return CSSToken::Url(value);
                }

                // "whitespace"
                Some(c) if is_whitespace(c) => {
                    self.consume_whitespace();
                    match self.peek() {
                        Some(')') => {
                            self.consume();
                            return CSSToken::Url(value);
                        }
                        None => {
                            return CSSToken::Url(value);
                        }
                        _ => {
                            self.consume_bad_url_remnants();
                            return CSSToken::BadUrl;
                        }
                    }
                }

                // "U+0022 QUOTATION MARK (")", U+0027 APOSTROPHE ('), U+0028 LEFT PARENTHESIS (()"
                // or "non-printable code point"
                // "This is a parse error. Consume the remnants of a bad url, create a
                // <bad-url-token>, and return it."
                Some('"') | Some('\'') | Some('(') => {
                    self.consume_bad_url_remnants();
                    return CSSToken::BadUrl;
                }

                // "U+005C REVERSE SOLIDUS (\)"
                Some('\\') => {
                    if self.is_valid_escape(Some('\\'), self.peek()) {
                        if let Some(c) = self.consume_escaped_code_point() {
                            value.push(c);
                        }
                    } else {
                        self.consume_bad_url_remnants();
                        return CSSToken::BadUrl;
                    }
                }

                // "anything else"
                // "Append the current input code point to the <url-token>'s value."
                Some(c) => {
                    value.push(c);
                }
            }
        }
    }

    /// [§ 4.3.14 Consume the remnants of a bad url](https://www.w3.org/TR/css-syntax-3/#consume-remnants-of-bad-url)
    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.consume() {
                Some(')') | None => return,
                Some('\\') => {
                    if self.is_valid_escape(Some('\\'), self.peek()) {
                        self.consume_escaped_code_point();
                    }
                }
                _ => continue,
            }
        }
    }

    /// [§ 4.3.11 Consume an ident sequence](https://www.w3.org/TR/css-syntax-3/#consume-name)
    fn consume_ident_sequence(&mut self) -> String {
        // "Let result initially be an empty string."
        let mut result = String::new();

        loop {
            match self.consume() {
                // "ident code point"
                // "Append the code point to result."
                Some(c) if is_ident_code_point(c) => {
                    result.push(c);
                }

                // "the stream starts with a valid escape"
                Some('\\') if self.is_valid_escape(Some('\\'), self.peek()) => {
                    // "Consume an escaped code point. Append the returned code point to result."
                    if let Some(c) = self.consume_escaped_code_point() {
                        result.push(c);
                    }
                }

                // "anything else"
                // "Reconsume the current input code point. Return result."
                Some(_) => {
                    self.reconsume();
                    return result;
                }

                None => return result,
            }
        }
    }

    /// [§ 4.3.12 Consume a number](https://www.w3.org/TR/css-syntax-3/#consume-number)
    fn consume_number(&mut self) -> (f64, Option<i64>, NumericType) {
        // "Initially set type to 'integer'. Let repr be the empty string."
        let mut numeric_type = NumericType::Integer;
        let mut repr = String::new();

        // "If the next input code point is U+002B PLUS SIGN (+) or U+002D HYPHEN-MINUS (-),
        // consume it and append it to repr."
        if self.peek() == Some('+') || self.peek() == Some('-') {
            repr.push(self.consume().unwrap());
        }

        // "While the next input code point is a digit, consume it and append it to repr."
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            repr.push(self.consume().unwrap());
        }

        // "If the next 2 input code points are U+002E FULL STOP (.) followed by a digit..."
        if self.peek() == Some('.')
            && self.peek_at(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            // "Consume them. Append them to repr. Set type to 'number'."
            repr.push(self.consume().unwrap()); // .
            repr.push(self.consume().unwrap()); // digit
            numeric_type = NumericType::Number;

            // "While the next input code point is a digit, consume it and append it to repr."
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                repr.push(self.consume().unwrap());
            }
        }

        // "If the next 2 or 3 input code points are U+0045 LATIN CAPITAL LETTER E (E)
        // or U+0065 LATIN SMALL LETTER E (e), optionally followed by U+002D HYPHEN-MINUS (-)
        // or U+002B PLUS SIGN (+), followed by a digit..."
        if self.peek() == Some('e') || self.peek() == Some('E') {
            let next = self.peek_at(1);
            let has_sign = next == Some('+') || next == Some('-');
            let digit_pos = if has_sign { 2 } else { 1 };

            if self
                .peek_at(digit_pos)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                repr.push(self.consume().unwrap()); // e or E
                if has_sign {
                    repr.push(self.consume().unwrap()); // + or -
                }
                repr.push(self.consume().unwrap()); // digit
                numeric_type = NumericType::Number;

                while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    repr.push(self.consume().unwrap());
                }
            }
        }

        // "Convert repr to a number, and set the value to the returned value."
        let value: f64 = repr.parse().unwrap_or(0.0);
        let int_value = if numeric_type == NumericType::Integer {
            repr.parse().ok()
        } else {
            None
        };

        (value, int_value, numeric_type)
    }

    /// [§ 4.3.13 Consume an escaped code point](https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point)
    fn consume_escaped_code_point(&mut self) -> Option<char> {
        match self.consume() {
            // "hex digit"
            Some(c) if c.is_ascii_hexdigit() => {
                let mut hex = c.to_string();
                // "Consume as many hex digits as possible, but no more than 5."
                for _ in 0..5 {
                    if self.peek().map(|c| c.is_ascii_hexdigit()).unwrap_or(false) {
                        hex.push(self.consume().unwrap());
                    } else {
                        break;
                    }
                }
                // "If the next input code point is whitespace, consume it."
                if self.peek().map(is_whitespace).unwrap_or(false) {
                    self.consume();
                }
                // "Interpret the hex digits as a hexadecimal number."
                let code_point = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                // "If this number is zero, or is for a surrogate, or is greater than the
                // maximum allowed code point, return U+FFFD REPLACEMENT CHARACTER."
                if code_point == 0
                    || (0xD800..=0xDFFF).contains(&code_point)
                    || code_point > 0x10FFFF
                {
                    Some('\u{FFFD}')
                } else {
                    char::from_u32(code_point)
                }
            }
            // "EOF"
            // "This is a parse error. Return U+FFFD REPLACEMENT CHARACTER."
            None => Some('\u{FFFD}'),
            // "anything else"
            // "Return the current input code point."
            Some(c) => Some(c),
        }
    }

    /// [§ 4.3.8 Check if two code points are a valid escape](https://www.w3.org/TR/css-syntax-3/#starts-with-a-valid-escape)
    fn is_valid_escape(&self, first: Option<char>, second: Option<char>) -> bool {
        // "If the first code point is not U+005C REVERSE SOLIDUS (\), return false."
        if first != Some('\\') {
            return false;
        }
        // "Otherwise, if the second code point is a newline, return false."
        if second == Some('\n') {
            return false;
        }
        // "Otherwise, return true."
        true
    }

    /// [§ 4.3.9 Check if three code points would start an ident sequence](https://www.w3.org/TR/css-syntax-3/#would-start-an-identifier)
    fn would_start_ident_sequence(&self) -> bool {
        self.would_start_ident_sequence_with(self.peek())
    }

    fn would_start_ident_sequence_with(&self, first: Option<char>) -> bool {
        match first {
            // "U+002D HYPHEN-MINUS"
            Some('-') => {
                let second = self.peek_at(1);
                // "If the second code point is an ident-start code point or a U+002D HYPHEN-MINUS,
                // or the second and third code points are a valid escape, return true."
                second.map(is_ident_start_code_point).unwrap_or(false)
                    || second == Some('-')
                    || self.is_valid_escape(second, self.peek_at(2))
            }
            // "ident-start code point"
            Some(c) if is_ident_start_code_point(c) => true,
            // "U+005C REVERSE SOLIDUS (\)"
            Some('\\') => self.is_valid_escape(Some('\\'), self.peek_at(1)),
            // "anything else"
            _ => false,
        }
    }

    /// [§ 4.3.10 Check if three code points would start a number](https://www.w3.org/TR/css-syntax-3/#starts-with-a-number)
    fn would_start_number(&self) -> bool {
        match self.peek() {
            // "U+002B PLUS SIGN (+)" or "U+002D HYPHEN-MINUS (-)"
            Some('+') | Some('-') => {
                let second = self.peek_at(1);
                // "If the second code point is a digit, return true."
                if second.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    return true;
                }
                // "Otherwise, if the second code point is U+002E FULL STOP (.) and the
                // third code point is a digit, return true."
                if second == Some('.') {
                    return self.peek_at(2).map(|c| c.is_ascii_digit()).unwrap_or(false);
                }
                false
            }
            // "U+002E FULL STOP (.)"
            Some('.') => self.peek_at(1).map(|c| c.is_ascii_digit()).unwrap_or(false),
            // "digit"
            Some(c) if c.is_ascii_digit() => true,
            // "anything else"
            _ => false,
        }
    }

    // Helper methods

    /// Consume and return the next character.
    fn consume(&mut self) -> Option<char> {
        if self.position < self.input.len() {
            let c = self.input[self.position];
            self.position += 1;
            Some(c)
        } else {
            None
        }
    }

    /// Put back the last consumed character.
    fn reconsume(&mut self) {
        if self.position > 0 {
            self.position -= 1;
        }
    }

    /// Peek at the next character without consuming it.
    fn peek(&self) -> Option<char> {
        self.peek_at(0)
    }

    /// Peek at a character at an offset from current position.
    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.position + offset).copied()
    }
}

/// [§ 4.2 Definitions - whitespace](https://www.w3.org/TR/css-syntax-3/#whitespace)
///
/// "A newline, U+0009 CHARACTER TABULATION, or U+0020 SPACE."
fn is_whitespace(c: char) -> bool {
    matches!(c, '\n' | '\t' | ' ' | '\r' | '\x0C')
}

/// [§ 4.2 Definitions - ident-start code point](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
///
/// "A letter, a non-ASCII code point, or U+005F LOW LINE (_)."
fn is_ident_start_code_point(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

/// [§ 4.2 Definitions - ident code point](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
///
/// "An ident-start code point, a digit, or U+002D HYPHEN-MINUS (-)."
fn is_ident_code_point(c: char) -> bool {
    is_ident_start_code_point(c) || c.is_ascii_digit() || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to tokenize a string and return the tokens
    fn tokenize(input: &str) -> Vec<CSSToken> {
        let mut tokenizer = CSSTokenizer::new(input);
        tokenizer.run();
        tokenizer.into_tokens()
    }

    #[test]
    fn test_whitespace() {
        let tokens = tokenize("   \t\n  ");
        assert_eq!(tokens.len(), 2); // whitespace + EOF
        assert!(matches!(tokens[0], CSSToken::Whitespace));
        assert!(matches!(tokens[1], CSSToken::EOF));
    }

    #[test]
    fn test_ident() {
        let tokens = tokenize("color");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "color"),
            _ => panic!("Expected Ident token"),
        }
    }

    #[test]
    fn test_ident_with_hyphen() {
        let tokens = tokenize("background-color");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "background-color"),
            _ => panic!("Expected Ident token"),
        }
    }

    #[test]
    fn test_ident_with_underscore() {
        let tokens = tokenize("_private");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "_private"),
            _ => panic!("Expected Ident token"),
        }
    }

    #[test]
    fn test_function() {
        let tokens = tokenize("rgb(");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Function(name) => assert_eq!(name, "rgb"),
            _ => panic!("Expected Function token"),
        }
    }

    #[test]
    fn test_at_keyword() {
        let tokens = tokenize("@media");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::AtKeyword(name) => assert_eq!(name, "media"),
            _ => panic!("Expected AtKeyword token"),
        }
    }

    #[test]
    fn test_hash_id() {
        let tokens = tokenize("#header");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Hash { value, hash_type } => {
                assert_eq!(value, "header");
                assert_eq!(*hash_type, HashType::Id);
            }
            _ => panic!("Expected Hash token"),
        }
    }

    #[test]
    fn test_hash_hex_color() {
        // #ff0000 starts with 'f' which is an ident-start code point,
        // so it's treated as an id-type hash per the spec
        let tokens = tokenize("#ff0000");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Hash { value, hash_type } => {
                assert_eq!(value, "ff0000");
                assert_eq!(*hash_type, HashType::Id);
            }
            _ => panic!("Expected Hash token"),
        }
    }

    #[test]
    fn test_hash_numeric_unrestricted() {
        // #123 starts with a digit, which is NOT an ident-start code point,
        // so it's unrestricted type
        let tokens = tokenize("#123");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Hash { value, hash_type } => {
                assert_eq!(value, "123");
                assert_eq!(*hash_type, HashType::Unrestricted);
            }
            _ => panic!("Expected Hash token"),
        }
    }

    #[test]
    fn test_string_double_quote() {
        let tokens = tokenize("\"hello world\"");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::String(value) => assert_eq!(value, "hello world"),
            _ => panic!("Expected String token"),
        }
    }

    #[test]
    fn test_string_single_quote() {
        let tokens = tokenize("'hello world'");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::String(value) => assert_eq!(value, "hello world"),
            _ => panic!("Expected String token"),
        }
    }

    #[test]
    fn test_integer() {
        let tokens = tokenize("42");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Number { value, int_value, numeric_type } => {
                assert_eq!(*value, 42.0);
                assert_eq!(*int_value, Some(42));
                assert_eq!(*numeric_type, NumericType::Integer);
            }
            _ => panic!("Expected Number token"),
        }
    }

    #[test]
    fn test_negative_integer() {
        let tokens = tokenize("-10");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Number { value, int_value, numeric_type } => {
                assert_eq!(*value, -10.0);
                assert_eq!(*int_value, Some(-10));
                assert_eq!(*numeric_type, NumericType::Integer);
            }
            _ => panic!("Expected Number token"),
        }
    }

    #[test]
    fn test_float() {
        let tokens = tokenize("3.14");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Number { value, numeric_type, .. } => {
                assert!((value - 3.14).abs() < 0.001);
                assert_eq!(*numeric_type, NumericType::Number);
            }
            _ => panic!("Expected Number token"),
        }
    }

    #[test]
    fn test_percentage() {
        let tokens = tokenize("50%");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Percentage { value, .. } => {
                assert_eq!(*value, 50.0);
            }
            _ => panic!("Expected Percentage token"),
        }
    }

    #[test]
    fn test_dimension_px() {
        let tokens = tokenize("16px");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Dimension { value, unit, .. } => {
                assert_eq!(*value, 16.0);
                assert_eq!(unit, "px");
            }
            _ => panic!("Expected Dimension token"),
        }
    }

    #[test]
    fn test_dimension_em() {
        let tokens = tokenize("1.5em");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Dimension { value, unit, .. } => {
                assert!((value - 1.5).abs() < 0.001);
                assert_eq!(unit, "em");
            }
            _ => panic!("Expected Dimension token"),
        }
    }

    #[test]
    fn test_colon() {
        let tokens = tokenize(":");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], CSSToken::Colon));
    }

    #[test]
    fn test_semicolon() {
        let tokens = tokenize(";");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], CSSToken::Semicolon));
    }

    #[test]
    fn test_comma() {
        let tokens = tokenize(",");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], CSSToken::Comma));
    }

    #[test]
    fn test_braces() {
        let tokens = tokenize("{}");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], CSSToken::LeftBrace));
        assert!(matches!(tokens[1], CSSToken::RightBrace));
    }

    #[test]
    fn test_brackets() {
        let tokens = tokenize("[]");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], CSSToken::LeftBracket));
        assert!(matches!(tokens[1], CSSToken::RightBracket));
    }

    #[test]
    fn test_parens() {
        let tokens = tokenize("()");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], CSSToken::LeftParen));
        assert!(matches!(tokens[1], CSSToken::RightParen));
    }

    #[test]
    fn test_comment() {
        let tokens = tokenize("/* comment */ color");
        assert_eq!(tokens.len(), 3); // whitespace + ident + EOF
        assert!(matches!(tokens[0], CSSToken::Whitespace));
        match &tokens[1] {
            CSSToken::Ident(name) => assert_eq!(name, "color"),
            _ => panic!("Expected Ident token"),
        }
    }

    #[test]
    fn test_cdo_cdc() {
        let tokens = tokenize("<!-- -->");
        assert_eq!(tokens.len(), 4); // CDO + whitespace + CDC + EOF
        assert!(matches!(tokens[0], CSSToken::CDO));
        assert!(matches!(tokens[1], CSSToken::Whitespace));
        assert!(matches!(tokens[2], CSSToken::CDC));
    }

    #[test]
    fn test_url_unquoted() {
        let tokens = tokenize("url(image.png)");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Url(value) => assert_eq!(value, "image.png"),
            _ => panic!("Expected Url token, got {:?}", tokens[0]),
        }
    }

    #[test]
    fn test_simple_rule() {
        let tokens = tokenize("color: red;");
        // color, :, whitespace, red, ;, EOF
        assert_eq!(tokens.len(), 6);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "color"),
            _ => panic!("Expected Ident token"),
        }
        assert!(matches!(tokens[1], CSSToken::Colon));
        assert!(matches!(tokens[2], CSSToken::Whitespace));
        match &tokens[3] {
            CSSToken::Ident(name) => assert_eq!(name, "red"),
            _ => panic!("Expected Ident token"),
        }
        assert!(matches!(tokens[4], CSSToken::Semicolon));
    }

    #[test]
    fn test_selector_and_block() {
        let tokens = tokenize("body { }");
        // body, whitespace, {, whitespace, }, EOF
        assert_eq!(tokens.len(), 6);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "body"),
            _ => panic!("Expected Ident token"),
        }
        assert!(matches!(tokens[2], CSSToken::LeftBrace));
        assert!(matches!(tokens[4], CSSToken::RightBrace));
    }

    #[test]
    fn test_class_selector() {
        let tokens = tokenize(".container");
        assert_eq!(tokens.len(), 3); // delim(.) + ident + EOF
        assert!(matches!(tokens[0], CSSToken::Delim('.')));
        match &tokens[1] {
            CSSToken::Ident(name) => assert_eq!(name, "container"),
            _ => panic!("Expected Ident token"),
        }
    }

    #[test]
    fn test_escaped_character() {
        let tokens = tokenize("\\41 ");  // \41 is 'A' in hex
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Ident(name) => assert_eq!(name, "A"),
            _ => panic!("Expected Ident token with escaped char"),
        }
    }

    #[test]
    fn test_scientific_notation() {
        let tokens = tokenize("1e10");
        assert_eq!(tokens.len(), 2);
        match &tokens[0] {
            CSSToken::Number { value, numeric_type, .. } => {
                assert_eq!(*value, 1e10);
                assert_eq!(*numeric_type, NumericType::Number);
            }
            _ => panic!("Expected Number token"),
        }
    }
}
