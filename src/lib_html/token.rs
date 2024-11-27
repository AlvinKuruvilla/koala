use core::fmt;

use strum_macros::Display;

/// The types of tokens the tokenizer produces
#[derive(Display, PartialEq)]
pub enum HTMLTokenType {
    Invalid,
    DOCTYPE,
    StartTag,
    EndTag,
    Comment,
    Character,
    EndOfFile,
}
pub struct TokenAttribute {
    name: String,
    value: String,
}
impl TokenAttribute {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
    // TODO: The clone is sad here
    pub fn get_attribute_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_attribute_value(&self) -> String {
        self.value.clone()
    }
    pub fn set_value(&mut self, new_value: String) {
        self.value = new_value
    }
}
/// Token Type DOCTYPE
#[derive(Default, Clone)]
pub struct DoctypeData {
    name: String,
    doctype_public_identifier: String,
    system_public_identifier: String,
    force_quirks_flag: bool,
}
impl DoctypeData {
    pub fn set_name(&mut self, name: String) {
        self.name = name
    }
    pub fn append_character_to_name(&mut self, ch: char) {
        self.name.push(ch);
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
}
impl fmt::Display for DoctypeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
/// Token Type Start and End Tag
#[derive(Default)]
pub struct StartOrEndTagData {
    text_name: String,
    self_closing: bool,
    attributes: Vec<TokenAttribute>,
}
impl StartOrEndTagData {
    pub fn set_self_closing_flag(&mut self, flag: bool) {
        self.self_closing = flag;
    }
}
#[derive(Default)]
pub struct CommentOrCharacterTagData {
    data: String,
}

pub struct HTMLToken {
    token_type: HTMLTokenType,
    doctype_data: DoctypeData,
    start_or_end_tag_data: StartOrEndTagData,
    comment_or_character_data: CommentOrCharacterTagData,
}
impl fmt::Display for HTMLToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.token_type, self.doctype_data)
    }
}
impl HTMLToken {
    pub fn new(token_type: HTMLTokenType) -> Self {
        Self {
            token_type,
            doctype_data: DoctypeData::default(),
            start_or_end_tag_data: StartOrEndTagData::default(),
            comment_or_character_data: CommentOrCharacterTagData::default(),
        }
    }
    pub fn set_token_type(&mut self, token_type: HTMLTokenType) {
        self.token_type = token_type
    }
    pub fn doctype_data(&mut self) -> &mut DoctypeData {
        &mut self.doctype_data
    }
    pub fn start_or_end_tag_data(&mut self) -> &mut StartOrEndTagData {
        &mut self.start_or_end_tag_data
    }
    pub fn is_eof(&self) -> bool {
        if self.token_type == HTMLTokenType::EndOfFile {
            return true;
        }
        false
    }
}
