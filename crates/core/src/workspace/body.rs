//! A page body as BlockNote 0.55's Yjs binding stores it. See `ARCHITECTURE.md`, "Page bodies".

use std::fmt;

use yrs::types::text::YChange;
use yrs::{
    Any, Array, ArrayRef, GetString, Map, MapRef, Out, ReadTxn, Text, Xml, XmlFragment, XmlOut,
};

use super::records::PageId;

/// The page doc's root: one Y.XmlFragment, the ProseMirror document's children.
pub const BODY_ROOT: &str = "prosemirror";

pub(crate) mod node {
    pub const BLOCK_GROUP: &str = "blockGroup";
    pub const BLOCK_CONTAINER: &str = "blockContainer";
    pub const PAGE_LINK: &str = "pageLink";
    pub const HARD_BREAK: &str = "hardBreak";
    pub const IMAGE: &str = "image";
    pub const ID: &str = "id";
    pub const TARGET_PAGE_ID: &str = "targetPageId";
    pub const CAPTION: &str = "caption";
}

/// A block's stable ID, the `id` attribute of its blockContainer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(String);

impl BlockId {
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        (!id.is_empty()).then_some(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One block's own content, without its children's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub segments: Vec<Segment>,
}

/// Text, or a page link that stores only its target (LINK-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Link(PageId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyError {
    /// The doc has content under another root and none under `prosemirror`.
    UnrecognizedRoot(String),
    MissingBlockId,
    InvalidPageLink,
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrecognizedRoot(root) => {
                write!(f, "the body is under root {root:?}, not {BODY_ROOT:?}")
            }
            Self::MissingBlockId => f.write_str("a blockContainer has no string id"),
            Self::InvalidPageLink => f.write_str("a pageLink has no valid targetPageId"),
        }
    }
}

impl std::error::Error for BodyError {}

/// Every block in document order, parents before their children. A doc with no content
/// has no blocks.
pub fn read_body<T: ReadTxn>(txn: &T) -> Result<Vec<Block>, BodyError> {
    let root = txn.get_xml_fragment(BODY_ROOT);
    if root.as_ref().is_none_or(|root| root.len(txn) == 0) {
        if let Some(name) = populated_root(txn) {
            return Err(BodyError::UnrecognizedRoot(name));
        }
        return Ok(Vec::new());
    }
    let mut blocks = Vec::new();
    let mut stack: Vec<XmlOut> = children(txn, &root.expect("checked above"));
    stack.reverse();
    while let Some(node) = stack.pop() {
        let XmlOut::Element(element) = node else {
            continue;
        };
        if element.tag().as_ref() != node::BLOCK_CONTAINER {
            let mut inner = children(txn, &element);
            inner.reverse();
            stack.extend(inner);
            continue;
        }
        let id = match element.get_attribute(txn, node::ID) {
            Some(Out::Any(Any::String(id))) => BlockId::new(id.to_string()),
            _ => None,
        }
        .ok_or(BodyError::MissingBlockId)?;
        let mut segments = Segments::default();
        let mut nested = Vec::new();
        for child in children(txn, &element) {
            match child {
                XmlOut::Element(group) if group.tag().as_ref() == node::BLOCK_GROUP => {
                    nested.extend(children(txn, &group))
                }
                content => segments.content(txn, content)?,
            }
        }
        blocks.push(Block {
            id,
            segments: segments.0,
        });
        nested.reverse();
        stack.extend(nested);
    }
    Ok(blocks)
}

/// The rendered text of `segments`: each link becomes `label(target)`.
pub fn render(segments: &[Segment], mut label: impl FnMut(&PageId) -> String) -> String {
    let mut text = String::new();
    for segment in segments {
        match segment {
            Segment::Text(t) => text.push_str(t),
            Segment::Link(target) => text.push_str(&label(target)),
        }
    }
    text
}

fn children<T: ReadTxn>(txn: &T, parent: &impl XmlFragment) -> Vec<XmlOut> {
    parent.children(txn).collect()
}

fn populated_root<T: ReadTxn>(txn: &T) -> Option<String> {
    txn.root_refs().find_map(|(name, value)| {
        let populated = match value {
            Out::UndefinedRef(branch) => {
                ArrayRef::from(branch).len(txn) > 0 || MapRef::from(branch).len(txn) > 0
            }
            Out::YText(text) => !text.get_string(txn).is_empty(),
            Out::YArray(array) => array.len(txn) > 0,
            Out::YMap(map) => map.len(txn) > 0,
            Out::YXmlFragment(fragment) => fragment.len(txn) > 0,
            Out::YXmlElement(element) => element.len(txn) > 0,
            Out::YXmlText(text) => !text.get_string(txn).is_empty(),
            _ => false,
        };
        (populated && name != BODY_ROOT).then(|| name.to_owned())
    })
}

#[derive(Default)]
struct Segments(Vec<Segment>);

impl Segments {
    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.0.last_mut() {
            Some(Segment::Text(last)) => last.push_str(text),
            _ => self.0.push(Segment::Text(text.to_owned())),
        }
    }

    fn content<T: ReadTxn>(&mut self, txn: &T, content: XmlOut) -> Result<(), BodyError> {
        let mut stack = vec![content];
        while let Some(node) = stack.pop() {
            match node {
                XmlOut::Text(text) => {
                    for chunk in text.diff(txn, YChange::identity) {
                        if let Out::Any(Any::String(s)) = chunk.insert {
                            self.push_text(&s);
                        }
                    }
                }
                XmlOut::Element(element) => match element.tag().as_ref() {
                    node::HARD_BREAK => self.push_text("\n"),
                    node::PAGE_LINK => {
                        let target = match element.get_attribute(txn, node::TARGET_PAGE_ID) {
                            Some(Out::Any(Any::String(id))) => PageId::new(id.to_string()).ok(),
                            _ => None,
                        };
                        self.0
                            .push(Segment::Link(target.ok_or(BodyError::InvalidPageLink)?));
                    }
                    tag => {
                        if tag == node::IMAGE
                            && let Some(Out::Any(Any::String(caption))) =
                                element.get_attribute(txn, node::CAPTION)
                        {
                            self.push_text(&caption);
                        }
                        let mut inner = children(txn, &element);
                        inner.reverse();
                        stack.extend(inner);
                    }
                },
                XmlOut::Fragment(_) => {}
            }
        }
        Ok(())
    }
}
