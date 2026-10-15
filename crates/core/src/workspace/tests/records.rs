use std::collections::{BTreeMap, HashMap};

use yrs::{Any, Array, Map, MapPrelim, Out, ReadTxn, Transact, WriteTxn};

use super::{doc_of, page};
use crate::storage::DocId;
use crate::workspace::fixture::{Editor, erase_page, put_page};
use crate::workspace::records::{
    Favorite, InvalidPageId, PageId, PageRecord, Placement, RecordError, Trash, read_pages,
};

fn record() -> PageRecord {
    PageRecord {
        title: "Projects".into(),
        placement: Placement {
            parent: Some(page("root")),
            order: 0,
        },
        favorite: Some(Favorite { order: 3 }),
        trash: Some(Trash {
            original_parent: None,
            original_order: 7,
            batch_id: "batch-1".into(),
            trashed_at: 1_760_000_000_123,
        }),
        created_at: 1_750_000_000_000,
        updated_at: (1 << 53) - 1,
    }
}

fn ws() -> DocId {
    DocId::workspace()
}

#[test]
fn page_ids_are_doc_ids_except_the_workspace() {
    assert_eq!(page("page-1").as_str(), "page-1");
    assert_eq!(
        PageId::new("workspace"),
        Err(InvalidPageId("workspace".into()))
    );
    assert_eq!(PageId::new("Page"), Err(InvalidPageId("Page".into())));
    assert_eq!(PageId::new(""), Err(InvalidPageId("".into())));
}

#[test]
fn every_field_round_trips() {
    let mut editor = Editor::new(1);
    let mut empty = record();
    empty.title = String::new();
    empty.placement = Placement {
        parent: None,
        order: 0,
    };
    empty.favorite = None;
    empty.trash = None;
    empty.created_at = 0;
    empty.updated_at = 0;
    let update = editor.edit(&ws(), |txn| {
        put_page(txn, &page("a"), &record());
        put_page(txn, &page("b"), &empty);
    });

    assert_eq!(
        read_pages(&doc_of(&update).transact()),
        Ok(BTreeMap::from([(page("a"), record()), (page("b"), empty)]))
    );
}

#[test]
fn a_doc_without_pages_has_no_records() {
    let mut editor = Editor::new(1);
    let update = editor.edit(&ws(), |txn| {
        txn.get_or_insert_map("other").insert(txn, "x", 1);
    });
    assert_eq!(read_pages(&doc_of(&update).transact()), Ok(BTreeMap::new()));
    assert_eq!(read_pages(&yrs::Doc::new().transact()), Ok(BTreeMap::new()));
}

#[test]
fn an_empty_pages_root_has_no_records() {
    let mut editor = Editor::new(1);
    let update = editor.edit(&ws(), |txn| {
        put_page(txn, &page("a"), &record());
        erase_page(txn, &page("a"));
    });
    let doc = doc_of(&update);
    assert!(doc.transact().get("pages").is_some());
    assert_eq!(read_pages(&doc.transact()), Ok(BTreeMap::new()));
    let typed = yrs::Doc::new();
    typed.get_or_insert_map("pages");
    assert_eq!(read_pages(&typed.transact()), Ok(BTreeMap::new()));
    let deleted_sequence = yrs::Doc::new();
    let array = deleted_sequence.get_or_insert_array("pages");
    array.insert(&mut deleted_sequence.transact_mut(), 0, 1);
    array.remove(&mut deleted_sequence.transact_mut(), 0);
    assert_eq!(
        read_pages(&deleted_sequence.transact()),
        Ok(BTreeMap::new())
    );
}

fn not_a_map(found: &str) -> Result<BTreeMap<PageId, PageRecord>, RecordError> {
    Err(RecordError {
        page: None,
        problem: format!("the pages root is not a Y.Map, found {found}"),
    })
}

#[test]
fn a_pages_root_with_sequence_content_is_a_typed_failure() {
    let doc = yrs::Doc::new();
    let array = doc.get_or_insert_array("pages");
    array.insert(&mut doc.transact_mut(), 0, "a");
    assert_eq!(read_pages(&doc.transact()), not_a_map("a sequence"));
    let replayed = doc_of(
        &doc.transact()
            .encode_state_as_update_v1(&Default::default()),
    );
    assert_eq!(read_pages(&replayed.transact()), not_a_map("a sequence"));

    let text = yrs::Doc::new();
    let typed = text.get_or_insert_text("pages");
    yrs::Text::push(&typed, &mut text.transact_mut(), "a");
    assert_eq!(read_pages(&text.transact()), not_a_map("a sequence"));

    let mut editor = Editor::new(1);
    editor.edit(&ws(), |txn| put_page(txn, &page("a"), &record()));
    let mut foreign = Editor::new(2);
    foreign.apply(&ws(), &editor.state(&ws()));
    let mixed = foreign.edit(&ws(), |txn| {
        txn.get_or_insert_array("pages").insert(txn, 0, "b");
    });
    let mut both = Editor::new(3);
    both.apply(&ws(), &editor.state(&ws()));
    both.apply(&ws(), &mixed);
    assert_eq!(
        read_pages(&doc_of(&both.state(&ws())).transact()),
        not_a_map("a sequence")
    );
}

#[test]
fn a_pages_root_of_another_shared_type_is_a_typed_failure() {
    let doc = yrs::Doc::new();
    let xml = doc.get_or_insert_xml_fragment("pages");
    yrs::XmlFragment::push_back(
        &xml,
        &mut doc.transact_mut(),
        yrs::XmlElementPrelim::empty("p"),
    );
    assert_eq!(read_pages(&doc.transact()), not_a_map("a sequence"));
    let empty = yrs::Doc::new();
    empty.get_or_insert_xml_fragment("pages");
    assert_eq!(read_pages(&empty.transact()), Ok(BTreeMap::new()));
}

#[test]
fn erasing_removes_the_record() {
    let mut editor = Editor::new(1);
    editor.edit(&ws(), |txn| {
        put_page(txn, &page("a"), &record());
        put_page(txn, &page("b"), &record());
    });
    editor.edit(&ws(), |txn| erase_page(txn, &page("a")));
    let doc = doc_of(&editor.state(&ws()));

    assert_eq!(
        read_pages(&doc.transact()),
        Ok(BTreeMap::from([(page("b"), record())]))
    );
}

#[test]
fn writers_keep_fields_they_do_not_know_and_readers_ignore_them() {
    let mut editor = Editor::new(1);
    editor.edit(&ws(), |txn| put_page(txn, &page("a"), &record()));
    editor.edit(&ws(), |txn| {
        let pages = txn.get_or_insert_map("pages");
        let Some(Out::YMap(a)) = pages.get(txn, "a") else {
            unreachable!()
        };
        a.insert(txn, "icon", "rocket");
        a.insert(
            txn,
            "placement",
            Any::from(HashMap::from([
                ("parentId".to_owned(), Any::from("root")),
                ("order".to_owned(), Any::from(0.0)),
                ("pinned".to_owned(), Any::from(true)),
            ])),
        );
    });
    let mut changed = record();
    changed.title = "Renamed".into();
    changed.placement.order = 4;
    editor.edit(&ws(), |txn| put_page(txn, &page("a"), &changed));
    let doc = doc_of(&editor.state(&ws()));
    let txn = doc.transact();
    assert_eq!(read_pages(&txn), Ok(BTreeMap::from([(page("a"), changed)])));
    let pages = txn.get_map("pages").unwrap();
    let Some(Out::YMap(a)) = pages.get(&txn, "a") else {
        unreachable!()
    };
    assert_eq!(a.get(&txn, "icon"), Some(Out::Any(Any::from("rocket"))));
    let Some(Out::Any(Any::Map(placement))) = a.get(&txn, "placement") else {
        unreachable!()
    };
    assert_eq!(placement.get("pinned"), Some(&Any::from(true)));
    assert_eq!(
        placement.get("order"),
        Some(&Any::Number(yrs::any::Number::Int(4)))
    );
}

fn read_with(f: impl FnOnce(&mut yrs::TransactionMut, &yrs::MapRef)) -> Result<(), RecordError> {
    let mut editor = Editor::new(1);
    let update = editor.edit(&ws(), |txn| {
        put_page(txn, &page("a"), &record());
        let pages = txn.get_or_insert_map("pages");
        let Some(Out::YMap(a)) = pages.get(txn, "a") else {
            unreachable!()
        };
        f(txn, &a);
    });
    read_pages(&doc_of(&update).transact()).map(drop)
}

fn problem(page: &str, problem: &str) -> Result<(), RecordError> {
    Err(RecordError {
        page: Some(page.into()),
        problem: problem.into(),
    })
}

fn object(fields: &[(&str, Any)]) -> Any {
    Any::from(
        fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<HashMap<_, _>>(),
    )
}

#[test]
fn malformed_known_fields_are_typed_failures() {
    assert_eq!(
        read_with(|txn, a| {
            a.insert(txn, "title", 5);
        }),
        problem("a", "title is not a string, found a number")
    );
    assert_eq!(
        read_with(|txn, a| {
            a.remove(txn, "createdAt");
        }),
        problem(
            "a",
            "createdAt is not an integer from 0 to 2^53 - 1, found nothing"
        )
    );
    for bad in [
        Any::from(-1.0),
        Any::from(1.5),
        Any::from(f64::NAN),
        Any::from(9_007_199_254_740_992.0),
        Any::from("3"),
    ] {
        assert_eq!(
            read_with(|txn, a| {
                a.insert(
                    txn,
                    "placement",
                    object(&[("parentId", Any::Null), ("order", bad.clone())]),
                );
            }),
            problem("a", "placement.order is not an integer from 0 to 2^53 - 1"),
            "{bad:?}"
        );
    }
    assert_eq!(
        read_with(|txn, a| {
            a.insert(
                txn,
                "placement",
                object(&[
                    ("parentId", Any::from("workspace")),
                    ("order", Any::from(0.0)),
                ]),
            );
        }),
        problem(
            "a",
            "placement.parentId is not a page ID (invalid page ID \"workspace\": use 1 to 64 of [0-9a-z-], not \"workspace\")"
        )
    );
    assert_eq!(
        read_with(|txn, a| {
            a.insert(txn, "favorite", "yes");
        }),
        problem("a", "favorite is not an object, found a string")
    );
    assert_eq!(
        read_with(|txn, a| {
            a.insert(
                txn,
                "trash",
                object(&[
                    ("originalParentId", Any::Null),
                    ("originalOrder", Any::from(0.0)),
                    ("batchId", Any::from("")),
                    ("trashedAt", Any::from(1.0)),
                ]),
            );
        }),
        problem("a", "trash.batchId is not a non-empty string")
    );
    assert_eq!(
        read_with(|txn, a| {
            a.insert(txn, "placement", MapPrelim::default());
        }),
        problem("a", "placement is not an object, found a shared type")
    );
    assert_eq!(
        read_with(|txn, _| {
            txn.get_or_insert_map("pages")
                .insert(txn, "b", "not a record");
        }),
        problem("b", "the record is not a Y.Map")
    );
    assert_eq!(
        read_with(|txn, _| {
            txn.get_or_insert_map("pages")
                .insert(txn, "Bad Key", MapPrelim::default());
        }),
        problem(
            "Bad Key",
            "the key is not a page ID (invalid page ID \"Bad Key\": use 1 to 64 of [0-9a-z-], not \"workspace\")"
        )
    );
}

#[test]
fn javascript_numbers_read_as_integers() {
    assert_eq!(
        read_with(|txn, a| {
            a.insert(txn, "updatedAt", Any::from(1_760_000_000_123.0));
            a.insert(txn, "createdAt", Any::Number(yrs::any::Number::Int(5)));
        }),
        Ok(())
    );
}
