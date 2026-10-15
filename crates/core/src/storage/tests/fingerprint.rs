use std::collections::HashMap;
use std::sync::Arc;

use yrs::{
    Any, Array, ArrayPrelim, Doc, Map, MapPrelim, ReadTxn, StateVector, Text, TextPrelim, Transact,
    TransactionMut, WriteTxn, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim,
};

use super::{ATTRIBUTED_PAGE, attributed_page, render_body};
use crate::storage::equality::{Fingerprint, Ops};
use crate::storage::testkit::{apply, doc_from, fingerprint};

fn decoded(doc: &Doc) -> Doc {
    let state = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    doc_from([state.as_slice()])
}

#[test]
fn an_attributed_page_matches_its_decoded_snapshot() {
    let sent = attributed_page();
    let held = doc_from(sent.iter().map(Vec::as_slice));
    let typed = Doc::new();
    typed.get_or_insert_xml_fragment("prosemirror");
    for update in &sent {
        apply(&typed, update);
    }
    for (case, source) in [("decoded updates", &held), ("typed root", &typed)] {
        let snapshot = decoded(source);
        assert_eq!(render_body(&snapshot), ATTRIBUTED_PAGE, "{case}");
        assert_eq!(fingerprint(source), fingerprint(&snapshot), "{case}");
        assert_eq!(
            fingerprint(&snapshot),
            fingerprint(&decoded(&snapshot)),
            "{case}"
        );
    }
}

fn mark(name: &str, value: Any) -> HashMap<Arc<str>, Any> {
    HashMap::from([(Arc::from(name), value)])
}

#[test]
fn every_shape_yrs_offers_matches_its_decoded_snapshot() {
    let doc = Doc::with_client_id(3);
    {
        let mut txn = doc.transact_mut();
        let text = txn.get_or_insert_text("text");
        text.insert(&mut txn, 0, "é and 🙂 ");
        let len = text.len(&txn);
        text.insert_with_attributes(&mut txn, len, "linked", mark("link", "https://a".into()));
        let len = text.len(&txn);
        text.insert_embed(&mut txn, len, Any::from(vec![Any::from("an embed")]));
        let len = text.len(&txn);
        let nested = text.insert_embed(&mut txn, len, ArrayPrelim::from([1, 2]));
        nested.push_back(&mut txn, "pushed");

        let embeds = txn.get_or_insert_text("embeds");
        embeds.insert_embed_with_attributes(&mut txn, 0, Any::from(1), mark("bold", true.into()));
        embeds.insert_embed(&mut txn, 1, Any::from(2));

        let array = txn.get_or_insert_array("array");
        array.push_back(&mut txn, Any::from(vec![1u8, 2, 3]));
        array.push_back(
            &mut txn,
            Any::from_json(r#"{"a": [1, {"b": null}]}"#).unwrap(),
        );
        let inner = array.push_back(&mut txn, TextPrelim::new("inner é"));
        inner.format(&mut txn, 0, 2, mark("bold", true.into()));
        array.push_back(&mut txn, MapPrelim::from([("k", Any::from(1))]));

        let map = txn.get_or_insert_map("map");
        map.insert(&mut txn, "number", 1.5);
        map.insert(&mut txn, "removed", "gone");
        map.remove(&mut txn, "removed");
        let nested = map.insert(&mut txn, "nested", MapPrelim::default());
        nested.insert(&mut txn, "list", ArrayPrelim::from([true, false]));

        let xml = txn.get_or_insert_xml_fragment("xml");
        let element = xml.push_back(&mut txn, XmlElementPrelim::empty("p"));
        element.insert_attribute(&mut txn, "a", "1");
        element.insert_attribute(&mut txn, "b", "2");
        element.insert_attribute(&mut txn, "c", "3");
        let run = element.push_back(&mut txn, XmlTextPrelim::new("x 🙂 y"));
        run.insert_attribute(&mut txn, "lang", "en");
        run.format(&mut txn, 0, 1, mark("code", Any::Null));
        run.format(
            &mut txn,
            2,
            4,
            mark("em", Any::from(HashMap::<String, Any>::new())),
        );
    }
    let snapshot = decoded(&doc);
    assert_eq!(fingerprint(&doc), fingerprint(&snapshot));
    assert_eq!(fingerprint(&snapshot), fingerprint(&decoded(&snapshot)));
}

/// Builds the same operations twice with client 1, so state vectors and delete sets match and
/// only what the operations hold can tell the docs apart.
fn twins(edit: impl Fn(&mut TransactionMut, bool)) -> (Doc, Doc) {
    let [a, b] = [false, true].map(|variant| {
        let doc = Doc::with_client_id(1);
        edit(&mut doc.transact_mut(), variant);
        doc
    });
    (a, b)
}

fn assert_told_apart(case: &str, edit: impl Fn(&mut TransactionMut, bool)) {
    let (a, b) = twins(edit);
    let (a_txn, b_txn) = (a.transact(), b.transact());
    assert_eq!(a_txn.snapshot(), b_txn.snapshot(), "{case}: same snapshot");
    assert_eq!(Ops::of_doc(&a_txn), Ops::of_doc(&b_txn), "{case}: same ops");
    drop((a_txn, b_txn));
    assert_eq!(
        fingerprint(&a),
        fingerprint(&decoded(&a)),
        "{case}: control"
    );
    assert_ne!(fingerprint(&a), fingerprint(&decoded(&b)), "{case}");
}

#[test]
fn the_fingerprint_tells_apart_docs_that_differ_only_in_content() {
    assert_told_apart("an attribute value", |txn, variant| {
        let xml = txn.get_or_insert_xml_fragment("prosemirror");
        let p = xml.push_back(txn, XmlElementPrelim::empty("paragraph"));
        p.insert_attribute(txn, "textColor", if variant { "red" } else { "blue" });
        p.insert_attribute(txn, "textAlignment", "left");
    });
    assert_told_apart("which mark", |txn, variant| {
        let xml = txn.get_or_insert_xml_fragment("prosemirror");
        let p = xml.push_back(txn, XmlElementPrelim::empty("paragraph"));
        let run = p.push_back(txn, XmlTextPrelim::new("bold"));
        let name = if variant { "italic" } else { "bold" };
        run.format(
            txn,
            0,
            4,
            mark(name, Any::from(HashMap::<String, Any>::new())),
        );
    });
    assert_told_apart("a mark missing from part of the text", |txn, variant| {
        let xml = txn.get_or_insert_xml_fragment("prosemirror");
        let p = xml.push_back(txn, XmlElementPrelim::empty("paragraph"));
        let run = p.push_back(txn, XmlTextPrelim::new("bold"));
        let len = if variant { 2 } else { 4 };
        run.format(
            txn,
            0,
            len,
            mark("bold", Any::from(HashMap::<String, Any>::new())),
        );
    });
    assert_told_apart("a mark's value", |txn, variant| {
        let text = txn.get_or_insert_text("text");
        let href = if variant { "https://b" } else { "https://a" };
        text.insert_with_attributes(txn, 0, "link", mark("link", href.into()));
    });
    assert_told_apart("a nested element's ID", |txn, variant| {
        let text = txn.get_or_insert_text("t");
        let xml = txn.get_or_insert_xml_fragment("x");
        if variant {
            xml.push_back(txn, XmlElementPrelim::empty("p"));
            text.insert(txn, 0, "a");
        } else {
            text.insert(txn, 0, "a");
            xml.push_back(txn, XmlElementPrelim::empty("p"));
        }
    });
    assert_told_apart("an element's tag", |txn, variant| {
        let xml = txn.get_or_insert_xml_fragment("prosemirror");
        xml.push_back(
            txn,
            XmlElementPrelim::empty(if variant { "heading" } else { "paragraph" }),
        );
    });
    assert_told_apart("a root's name", |txn, variant| {
        let xml = txn.get_or_insert_xml_fragment(if variant { "other" } else { "prosemirror" });
        xml.push_back(txn, XmlElementPrelim::empty("paragraph"));
    });
    assert_told_apart("a map entry", |txn, variant| {
        let map = txn.get_or_insert_map("meta");
        map.insert(txn, "status", if variant { "draft" } else { "final" });
    });
    assert_told_apart("an array value", |txn, variant| {
        let array = txn.get_or_insert_array("rows");
        array.push_back(txn, if variant { 2 } else { 1 });
    });
}

fn assert_not_compared(case: &str, edit: impl Fn(&mut TransactionMut, bool)) {
    let (a, b) = twins(edit);
    let (a_txn, b_txn) = (a.transact(), b.transact());
    assert_eq!(a_txn.snapshot(), b_txn.snapshot(), "{case}: same snapshot");
    assert_eq!(Ops::of_doc(&a_txn), Ops::of_doc(&b_txn), "{case}: same ops");
    drop((a_txn, b_txn));
    for (twin, doc) in [("a", &a), ("a decoded", &decoded(&a)), ("b", &b)] {
        let fingerprint = Fingerprint::of(&doc.transact(), doc.offset_kind());
        assert_eq!(fingerprint, None, "{case}: {twin}");
    }
}

/// Text runs can't show these differences, so the fingerprint refuses the doc, and compaction
/// leaves it alone.
#[test]
fn a_branch_whose_text_runs_hide_content_is_not_compared() {
    assert_not_compared("values beside text", |txn, variant| {
        txn.get_or_insert_array("mixed")
            .push_back(txn, if variant { 2 } else { 1 });
        txn.get_or_insert_text("mixed").insert(txn, 0, "é");
    });
    assert_not_compared("a string embed beside text", |txn, variant| {
        let text = txn.get_or_insert_text("text");
        text.insert(txn, 0, "é");
        let embed = if variant { "b" } else { "a" };
        text.insert_embed(txn, 0, Any::from(embed));
    });
}

#[test]
fn text_with_deleted_runs_matches_its_decoded_snapshot() {
    let doc = Doc::with_client_id(1);
    {
        let mut txn = doc.transact_mut();
        let text = txn.get_or_insert_text("text");
        text.insert(&mut txn, 0, "é and 🙂 and more");
        text.remove_range(&mut txn, 2, 5);
        let xml = txn.get_or_insert_xml_fragment("prosemirror");
        let p = xml.push_back(&mut txn, XmlElementPrelim::empty("paragraph"));
        let run = p.push_back(&mut txn, XmlTextPrelim::new("keep 🙂 drop"));
        run.remove_range(&mut txn, 9, 5);
        run.insert_embed(&mut txn, 0, Any::from(7));
    }
    let snapshot = decoded(&doc);
    assert_eq!(fingerprint(&doc), fingerprint(&snapshot));
    assert_eq!(fingerprint(&snapshot), fingerprint(&decoded(&snapshot)));
}
