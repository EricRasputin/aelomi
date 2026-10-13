use std::path::Path;

use yrs::{
    Any, GetString, ReadTxn, Text, Transact, WriteTxn, Xml, XmlElementPrelim, XmlFragment,
    XmlTextPrelim,
};

use super::{doc_of, page};
use crate::storage::DocId;
use crate::workspace::body::{Block, BlockId, BodyError, Segment, read_body};
use crate::workspace::fixture::{
    BlockSpec, Editor, Inline, block, link, paragraph, text, write_body,
};

fn t(s: &str) -> Segment {
    Segment::Text(s.to_owned())
}

fn l(id: &str) -> Segment {
    Segment::Link(page(id))
}

fn b(id: &str, segments: Vec<Segment>) -> Block {
    Block {
        id: BlockId::new(id).unwrap(),
        segments,
    }
}

/// What BlockNote 0.55.0 and y-prosemirror 1.3.7 wrote for `fixtures/index/generate-blocknote-body.mjs`.
fn expected_fixture_blocks() -> Vec<Block> {
    vec![
        b("block-heading", vec![t("Café roadmap")]),
        b("block-link", vec![t("see "), l("page-target"), t(" today")]),
        b(
            "block-twice",
            vec![
                l("page-target"),
                t(" and "),
                l("page-target"),
                t(" and "),
                l("page-other"),
            ],
        ),
        b("block-parent", vec![t("parent item")]),
        b("block-child", vec![t("child item")]),
        b("block-grandchild", vec![t("done "), l("page-other")]),
        b("block-numbered", vec![t("first step")]),
        b(
            "block-code",
            vec![t("fn main() {\n    let s = \"[[Not a link]]\";\n}")],
        ),
        b("block-break", vec![t("line one\nline two")]),
        b(
            "block-href",
            vec![t("read the docs and [[Literal brackets]]")],
        ),
        b("block-quote", vec![t("naïve 日本 quote")]),
        b("block-image", vec![t("Architecture diagram")]),
        b("block-divider", vec![]),
        b("block-empty", vec![]),
    ]
}

#[test]
fn the_real_blocknote_fixture_reads_as_its_blocks() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/index/blocknote-body.yupdate");
    let update = std::fs::read(path).unwrap();
    let doc = doc_of(&update);

    assert_eq!(read_body(&doc.transact()), Ok(expected_fixture_blocks()));
    assert_eq!(
        doc_of(&update)
            .transact()
            .encode_state_as_update_v1(&Default::default()),
        doc.transact()
            .encode_state_as_update_v1(&Default::default()),
        "reading changes nothing"
    );
}

#[test]
fn the_rust_builder_writes_the_layout_the_real_binding_writes() {
    let specs = vec![
        block(
            "block-heading",
            "heading",
            vec![
                text("Café "),
                Inline::Marked("roadmap".into(), "bold".into()),
            ],
        )
        .with_attr("level", Any::from(2.0)),
        paragraph(
            "block-link",
            vec![
                text("see "),
                link("page-target"),
                Inline::Marked(" today".into(), "italic".into()),
            ],
        ),
        paragraph(
            "block-twice",
            vec![
                link("page-target"),
                text(" and "),
                link("page-target"),
                text(" and "),
                link("page-other"),
            ],
        ),
        block("block-parent", "bulletListItem", vec![text("parent item")]).with_children(vec![
            block("block-child", "bulletListItem", vec![text("child item")]).with_children(vec![
                block(
                    "block-grandchild",
                    "checkListItem",
                    vec![text("done "), link("page-other")],
                )
                .with_attr("checked", true),
            ]),
        ]),
        block(
            "block-numbered",
            "numberedListItem",
            vec![text("first step")],
        ),
        block(
            "block-code",
            "codeBlock",
            vec![text("fn main() {\n    let s = \"[[Not a link]]\";\n}")],
        )
        .with_attr("language", "rust"),
        paragraph(
            "block-break",
            vec![text("line one"), Inline::HardBreak, text("line two")],
        ),
        paragraph(
            "block-href",
            vec![
                text("read the "),
                Inline::Marked("docs".into(), "link".into()),
                Inline::Marked(" and [[Literal brackets]]".into(), "code".into()),
            ],
        ),
        block("block-quote", "quote", vec![text("naïve 日本 quote")]),
        block("block-image", "image", vec![])
            .with_attr("caption", "Architecture diagram")
            .with_attr("url", "asset://0123abcd.png"),
        block("block-divider", "divider", vec![]),
        paragraph("block-empty", vec![]),
    ];
    let mut editor = Editor::new(1);
    let update = editor.edit(&DocId::new("p").unwrap(), |txn| write_body(txn, &specs));

    assert_eq!(
        read_body(&doc_of(&update).transact()),
        Ok(expected_fixture_blocks())
    );
}

fn body_of(f: impl FnOnce(&mut yrs::TransactionMut)) -> Result<Vec<Block>, BodyError> {
    let mut editor = Editor::new(1);
    let update = editor.edit(&DocId::new("p").unwrap(), f);
    read_body(&doc_of(&update).transact())
}

#[test]
fn an_empty_doc_has_no_blocks() {
    assert_eq!(body_of(|_| {}), Ok(vec![]));
    assert_eq!(body_of(|txn| write_body(txn, &[])), Ok(vec![]));
}

#[test]
fn content_under_another_root_is_refused_not_read_as_empty() {
    let refused = body_of(|txn| {
        let content = txn.get_or_insert_text("content");
        content.insert(txn, 0, "a paragraph the reader does not know");
    });
    assert_eq!(refused, Err(BodyError::UnrecognizedRoot("content".into())));

    let read = body_of(|txn| {
        write_body(txn, &[paragraph("a", vec![text("known")])]);
        let extra = txn.get_or_insert_text("comments");
        extra.insert(txn, 0, "an added root");
    });
    assert_eq!(read, Ok(vec![b("a", vec![t("known")])]));
}

#[test]
fn a_block_without_an_id_or_a_link_without_a_target_is_refused() {
    let no_id = body_of(|txn| {
        write_body(txn, &[paragraph("a", vec![text("x")])]);
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        group.insert(txn, 1, XmlElementPrelim::empty("blockContainer"));
    });
    assert_eq!(no_id, Err(BodyError::MissingBlockId));

    for target in ["", "workspace", "Not An ID"] {
        let bad_link = body_of(|txn| {
            write_body(txn, &[paragraph("a", vec![text("x")])]);
            let fragment = txn.get_or_insert_xml_fragment("prosemirror");
            let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
                unreachable!()
            };
            let Some(yrs::XmlOut::Element(container)) = group.get(txn, 0) else {
                unreachable!()
            };
            let Some(yrs::XmlOut::Element(content)) = container.get(txn, 0) else {
                unreachable!()
            };
            let link = content.insert(txn, 1, XmlElementPrelim::empty("pageLink"));
            link.insert_attribute(txn, "targetPageId", target);
        });
        assert_eq!(bad_link, Err(BodyError::InvalidPageLink), "{target:?}");
    }
}

#[test]
fn unknown_nodes_keep_their_text_and_attributes_are_not_text() {
    let read = body_of(|txn| {
        write_body(
            txn,
            &[BlockSpec {
                id: "a".into(),
                kind: "futureBlock".into(),
                attrs: vec![("url".into(), Any::from("https://example.com"))],
                content: vec![text("before ")],
                children: vec![],
            }],
        );
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        let Some(yrs::XmlOut::Element(container)) = group.get(txn, 0) else {
            unreachable!()
        };
        let Some(yrs::XmlOut::Element(content)) = container.get(txn, 0) else {
            unreachable!()
        };
        let mention = content.insert(txn, 1, XmlElementPrelim::empty("mention"));
        mention.insert(txn, 0, XmlTextPrelim::new("@someone"));
        let after = content.insert(txn, 2, XmlTextPrelim::new(""));
        after.insert(txn, 0, " after");
        assert_eq!(after.get_string(txn), " after");
    });
    assert_eq!(read, Ok(vec![b("a", vec![t("before @someone after")])]));
}
