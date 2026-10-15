use std::sync::Arc;
use std::time::{Duration, Instant};

use yrs::{WriteTxn, XmlFragment};

use super::{Docs, all, content, hit_ids, index_path, open, page, ready, ready_with_issues};
use crate::index::{Config, Index, PageIssue, PageProblem};
use crate::storage::DocId;
use crate::workspace::fixture::index_fixture;

#[test]
fn a_body_root_of_another_type_is_an_issue_and_every_other_page_still_reads() {
    use crate::workspace::records::{PageRecord, Placement};
    use yrs::{ReadTxn, Transact};
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let before = ready(opened.workspace.search(&all("roadmap", 50)).unwrap());
    let typed = DocId::new("typed-text").unwrap();
    let doc = yrs::Doc::new();
    let body = doc.get_or_insert_text("prosemirror");
    yrs::Text::insert(
        &body,
        &mut doc.transact_mut(),
        0,
        "plain kestrel words, café 😀",
    );
    let encoded = doc
        .transact()
        .encode_state_as_update_v1(&Default::default());
    opened.workspace.append(&typed, &encoded).unwrap();
    opened.edit_records(|records| {
        records.insert(
            page("typed-text"),
            PageRecord {
                title: "Typed".into(),
                placement: Placement {
                    parent: None,
                    order: 9,
                },
                favorite: None,
                trash: None,
                created_at: 900,
                updated_at: 900,
            },
        );
    });
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(5))
            .docs,
        []
    );
    let source = super::source_files(dir.path());
    opened.settle();

    let issue = PageIssue {
        page_id: page("typed-text"),
        problem: PageProblem::NotAFragment,
        indexed_blocks: 0,
    };
    let (hits, issues) = ready_with_issues(
        opened
            .workspace
            .search(&content("kestrel", 10, &[]))
            .unwrap(),
    );
    assert_eq!(hit_ids(&hits), ["aelomi"]);
    assert_eq!(issues, std::slice::from_ref(&issue));
    let (hits, issues) = ready_with_issues(opened.workspace.search(&all("roadmap", 50)).unwrap());
    assert_eq!(hits, before);
    assert_eq!(issues, [issue]);
    assert_eq!(super::source_files(dir.path()), source);
}

#[test]
fn a_body_under_another_root_or_not_a_yjs_update_is_named_with_no_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    opened.edit_body("projects", |txn| {
        let body = txn.get_or_insert_xml_fragment("prosemirror");
        let len = body.len(txn);
        body.remove_range(txn, 0, len);
        let elsewhere = txn.get_or_insert_text("content");
        yrs::Text::insert(&elsewhere, txn, 0, "project index elsewhere");
    });
    opened.settle();
    let (hits, issues) = ready_with_issues(
        opened
            .workspace
            .search(&content("project", 10, &[]))
            .unwrap(),
    );
    assert_eq!(hits, []);
    assert_eq!(
        issues,
        [PageIssue {
            page_id: page("projects"),
            problem: PageProblem::UnrecognizedRoot("content".into()),
            indexed_blocks: 0,
        }]
    );

    let mut docs = std::collections::BTreeMap::new();
    for (doc, update) in index_fixture() {
        if doc == DocId::workspace() {
            let merged = match docs.remove(&doc) {
                None => update,
                Some(old) => yrs::merge_updates_v1([old, update]).unwrap(),
            };
            docs.insert(doc, merged);
        } else {
            docs.insert(doc, update);
        }
    }
    docs.insert(DocId::new("daily").unwrap(), b"not a yjs update".to_vec());
    let other = tempfile::tempdir().unwrap();
    let index = Index::start(
        Arc::new(Docs(docs)),
        index_path(other.path()),
        Config::DEFAULT,
        Arc::new(|_| {}),
        Arc::default(),
    );
    assert!(index.shared().settle(Duration::from_secs(10)));
    let (hits, issues) = ready_with_issues(index.search(&content("cafe", 10, &[])).unwrap());
    assert_eq!(hit_ids(&hits), ["reading"]);
    assert_eq!(
        issues,
        [PageIssue {
            page_id: page("daily"),
            problem: PageProblem::Undecodable,
            indexed_blocks: 0,
        }]
    );
}
