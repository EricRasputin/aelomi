//! A page body as BlockNote 0.55's Yjs binding stores it. See `ARCHITECTURE.md`, "Page bodies".

use std::fmt;

use yrs::branch::{Branch, BranchPtr};
use yrs::types::text::YChange;
use yrs::{
    Any, Array, ArrayRef, GetString, Map, MapRef, Out, ReadTxn, Text, TextRef, Xml, XmlElementRef,
    XmlFragment, XmlOut,
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

/// The blocks a body reader could read, and the most serious problem it met, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Body {
    pub blocks: Vec<Block>,
    pub problem: Option<BodyProblem>,
}

/// A part of the body the reader skipped. Ordered most serious first.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyProblem {
    /// The doc has content under this other root and none under `prosemirror`, so it has no
    /// blocks.
    UnrecognizedRoot(String),
    /// The `prosemirror` root, or an element under it, holds content that is not XML, such as
    /// text or values another writer stored there. That content is skipped.
    NotAFragment,
    /// A blockContainer has no string `id`. Its own content is skipped and its children read.
    MissingBlockId,
    /// A `pageLink` has no valid `targetPageId`. It reads as a space.
    InvalidPageLink,
}

impl fmt::Display for BodyProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrecognizedRoot(root) => {
                write!(f, "the body is under root {root:?}, not {BODY_ROOT:?}")
            }
            Self::NotAFragment => write!(f, "the {BODY_ROOT:?} body holds content that is not XML"),
            Self::MissingBlockId => f.write_str("a blockContainer has no string id"),
            Self::InvalidPageLink => f.write_str("a pageLink has no valid targetPageId"),
        }
    }
}

impl Body {
    fn note(&mut self, problem: BodyProblem) {
        if self.problem.as_ref().is_none_or(|known| problem < *known) {
            self.problem = Some(problem);
        }
    }
}

/// Every readable block in document order, parents before their children. A doc with no
/// content has no blocks. A malformed part is skipped and named, never fails the whole body.
pub fn read_body<T: ReadTxn>(txn: &T) -> Body {
    let mut body = Body::default();
    let Some(root) = body_root(txn, &mut body) else {
        if let Some(name) = populated_root(txn) {
            body.note(BodyProblem::UnrecognizedRoot(name));
        }
        return body;
    };
    let mut stack = nodes(txn, root, &mut body);
    stack.reverse();
    while let Some(node) = stack.pop() {
        let XmlOut::Element(element) = node else {
            continue;
        };
        if element.tag().as_ref() != node::BLOCK_CONTAINER {
            let mut inner = children(txn, &element, &mut body);
            inner.reverse();
            stack.extend(inner);
            continue;
        }
        let id = match element.get_attribute(txn, node::ID) {
            Some(Out::Any(Any::String(id))) => BlockId::new(id.to_string()),
            _ => None,
        };
        let mut segments = Segments::default();
        let mut nested = Vec::new();
        for child in children(txn, &element, &mut body) {
            match child {
                XmlOut::Element(group) if group.tag().as_ref() == node::BLOCK_GROUP => {
                    nested.extend(children(txn, &group, &mut body))
                }
                content if id.is_some() => segments.content(txn, content, &mut body),
                _ => {}
            }
        }
        match id {
            Some(id) => body.blocks.push(Block {
                id,
                segments: segments.0,
            }),
            None => body.note(BodyProblem::MissingBlockId),
        }
        nested.reverse();
        stack.extend(nested);
    }
    body
}

/// The `prosemirror` root when it holds anything. A root that arrived in an update has no type
/// until someone asks for one, so its content decides, whatever type this doc gave it.
fn body_root<T: ReadTxn>(txn: &T, body: &mut Body) -> Option<BranchPtr> {
    let root = txn.get(BODY_ROOT)?;
    let branch: &Branch = match &root {
        Out::UndefinedRef(branch) => branch,
        Out::YXmlFragment(fragment) => fragment.as_ref(),
        Out::YText(text) => text.as_ref(),
        Out::YArray(array) => array.as_ref(),
        Out::YMap(map) => map.as_ref(),
        Out::YXmlElement(element) => element.as_ref(),
        Out::YXmlText(text) => text.as_ref(),
        Out::Any(_) | Out::YDoc(_) => return None,
    };
    let branch = BranchPtr::from(branch);
    let sequence = ArrayRef::from(branch).len(txn) > 0;
    let entries = MapRef::from(branch).len(txn) > 0;
    if entries {
        body.note(BodyProblem::NotAFragment);
    }
    (sequence || entries).then_some(branch)
}

/// Every XML node in `branch`'s sequence. yrs's own children iterator ends at the first value
/// that is not XML, which would drop the nodes after it unnamed, and its sequence iterator never
/// ends on a string with a multi-byte character, so a branch holding text is read as text runs.
fn nodes<T: ReadTxn>(txn: &T, branch: BranchPtr, body: &mut Body) -> Vec<XmlOut> {
    let text = TextRef::from(branch);
    if !text.get_string(txn).is_empty() {
        body.note(BodyProblem::NotAFragment);
        return text
            .diff(txn, YChange::identity)
            .into_iter()
            .filter_map(|run| xml(run.insert))
            .collect();
    }
    ArrayRef::from(branch)
        .iter(txn)
        .filter_map(|value| {
            let node = xml(value);
            if node.is_none() {
                body.note(BodyProblem::NotAFragment);
            }
            node
        })
        .collect()
}

fn xml(value: Out) -> Option<XmlOut> {
    match value {
        Out::YXmlElement(element) => Some(XmlOut::Element(element)),
        Out::YXmlText(text) => Some(XmlOut::Text(text)),
        _ => None,
    }
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

fn children<T: ReadTxn>(txn: &T, parent: &XmlElementRef, body: &mut Body) -> Vec<XmlOut> {
    nodes(txn, BranchPtr::from(parent.as_ref() as &Branch), body)
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

    fn content<T: ReadTxn>(&mut self, txn: &T, content: XmlOut, body: &mut Body) {
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
                    node::PAGE_LINK => match element.get_attribute(txn, node::TARGET_PAGE_ID) {
                        Some(Out::Any(Any::String(id))) => match PageId::new(id.to_string()) {
                            Ok(target) => self.0.push(Segment::Link(target)),
                            Err(_) => self.invalid_link(body),
                        },
                        _ => self.invalid_link(body),
                    },
                    tag => {
                        if tag == node::IMAGE
                            && let Some(Out::Any(Any::String(caption))) =
                                element.get_attribute(txn, node::CAPTION)
                        {
                            self.push_text(&caption);
                        }
                        let mut inner = children(txn, &element, body);
                        inner.reverse();
                        stack.extend(inner);
                    }
                },
                XmlOut::Fragment(_) => {}
            }
        }
    }

    fn invalid_link(&mut self, body: &mut Body) {
        self.push_text(" ");
        body.note(BodyProblem::InvalidPageLink);
    }
}
