use std::path::Path;

use yrs::{
    Any, GetString, ReadTxn, Text, Transact, WriteTxn, Xml, XmlElementPrelim, XmlFragment,
    XmlTextPrelim,
};

use super::{doc_of, page};
use crate::storage::DocId;
use crate::workspace::body::{Block, BlockId, Body, BodyProblem, Segment, read_body};
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

    assert_eq!(read_body(&doc.transact()), whole(expected_fixture_blocks()));
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
        whole(expected_fixture_blocks())
    );
}

fn whole(blocks: Vec<Block>) -> Body {
    Body {
        blocks,
        problem: None,
    }
}

fn body_of(f: impl FnOnce(&mut yrs::TransactionMut)) -> Body {
    let mut editor = Editor::new(1);
    let update = editor.edit(&DocId::new("p").unwrap(), f);
    read_body(&doc_of(&update).transact())
}

fn first_content(txn: &mut yrs::TransactionMut) -> yrs::XmlElementRef {
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
    content
}

#[test]
fn an_empty_doc_has_no_blocks() {
    assert_eq!(body_of(|_| {}), whole(vec![]));
    assert_eq!(body_of(|txn| write_body(txn, &[])), whole(vec![]));
}

#[test]
fn content_under_another_root_is_named_not_read_as_empty() {
    let refused = body_of(|txn| {
        let content = txn.get_or_insert_text("content");
        content.insert(txn, 0, "a paragraph the reader does not know");
    });
    assert_eq!(
        refused,
        Body {
            blocks: vec![],
            problem: Some(BodyProblem::UnrecognizedRoot("content".into())),
        }
    );

    let read = body_of(|txn| {
        write_body(txn, &[paragraph("a", vec![text("known")])]);
        let extra = txn.get_or_insert_text("comments");
        extra.insert(txn, 0, "an added root");
    });
    assert_eq!(read, whole(vec![b("a", vec![t("known")])]));
}

#[test]
fn a_block_without_an_id_is_skipped_and_its_children_are_read() {
    let read = body_of(|txn| {
        write_body(
            txn,
            &[
                paragraph("a", vec![text("first")]),
                paragraph("parent", vec![text("lost words")])
                    .with_children(vec![paragraph("child", vec![text("kept child")])]),
                paragraph("c", vec![text("last")]),
            ],
        );
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        let Some(yrs::XmlOut::Element(parent)) = group.get(txn, 1) else {
            unreachable!()
        };
        parent.remove_attribute(txn, &"id");
    });
    assert_eq!(
        read,
        Body {
            blocks: vec![
                b("a", vec![t("first")]),
                b("child", vec![t("kept child")]),
                b("c", vec![t("last")]),
            ],
            problem: Some(BodyProblem::MissingBlockId),
        }
    );
}

#[test]
fn a_link_without_a_valid_target_reads_as_a_space() {
    for target in ["", "workspace", "Not An ID"] {
        let read = body_of(|txn| {
            write_body(txn, &[paragraph("a", vec![text("left"), text("right")])]);
            let content = first_content(txn);
            let Some(yrs::XmlOut::Text(run)) = content.get(txn, 0) else {
                unreachable!()
            };
            run.remove_range(txn, 4, 5);
            content.insert(txn, 1, XmlTextPrelim::new("right"));
            let link = content.insert(txn, 1, XmlElementPrelim::empty("pageLink"));
            link.insert_attribute(txn, "targetPageId", target);
        });
        assert_eq!(
            read,
            Body {
                blocks: vec![b("a", vec![t("left right")])],
                problem: Some(BodyProblem::InvalidPageLink),
            },
            "{target:?}"
        );
    }
}

#[test]
fn the_most_serious_problem_is_named() {
    let read = body_of(|txn| {
        write_body(
            txn,
            &[
                paragraph("a", vec![text("x")]),
                paragraph("b", vec![text("y")]),
            ],
        );
        let content = first_content(txn);
        let link = content.insert(txn, 1, XmlElementPrelim::empty("pageLink"));
        link.insert_attribute(txn, "targetPageId", "Not An ID");
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        let Some(yrs::XmlOut::Element(second)) = group.get(txn, 1) else {
            unreachable!()
        };
        second.remove_attribute(txn, &"id");
    });
    assert_eq!(
        read,
        Body {
            blocks: vec![b("a", vec![t("x ")])],
            problem: Some(BodyProblem::MissingBlockId),
        }
    );
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
    assert_eq!(read, whole(vec![b("a", vec![t("before @someone after")])]));
}
