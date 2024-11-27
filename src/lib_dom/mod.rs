use std::collections::{HashMap, HashSet};

pub type AttributesMap = HashMap<String, String>;

#[derive(Debug)]
pub struct Node {
    pub children: Vec<Node>,
    pub node_type: NodeType,
}
#[derive(Debug)]
pub enum NodeType {
    Element(ElementData),
    Text(String),
}
#[derive(Debug)]
pub struct ElementData {
    pub tag_name: String,
    pub attrs: AttributesMap,
}

// Constructor functions for convenience:

pub fn text(data: String) -> Node {
    Node {
        children: vec![],
        node_type: NodeType::Text(data),
    }
}

pub fn elem(tag_name: String, attrs: AttributesMap, children: Vec<Node>) -> Node {
    Node {
        children,
        node_type: NodeType::Element(ElementData { tag_name, attrs }),
    }
}
impl ElementData {
    pub fn id(&self) -> Option<&String> {
        self.attrs.get("id")
    }

    pub fn classes(&self) -> HashSet<&str> {
        match self.attrs.get("class") {
            Some(classlist) => classlist.split(' ').collect(),
            None => HashSet::new(),
        }
    }
}
