use std::collections::{BTreeMap, BTreeSet};
use std::sync::Barrier;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use yrs::{Any, Map, ReadTxn, Transact, WriteTxn};

use super::{doc_of, page};
use crate::storage::{DocId, Store};
use crate::workspace::fixture::{Editor, put_page};
use crate::workspace::records::{PageId, PageRecord, Placement, read_pages};
use crate::workspace::{PageEdit, PageEditError, Workspace};

fn empty(dir: &std::path::Path) -> Workspace {
    Workspace::with_store(
        Store::open(dir).unwrap(),
        std::fs::canonicalize(dir).unwrap().join("index.sqlite"),
        |_| {},
    )
}
fn create(w: &Workspace, parent: Option<PageId>, title: &str) -> PageId {
    w.edit_page(PageEdit::Create {
        parent,
        title: title.into(),
    })
    .unwrap()
    .created_page()
    .unwrap()
}
fn records(w: &Workspace) -> BTreeMap<PageId, PageRecord> {
    read_pages(&doc_of(&w.load(&DocId::workspace()).unwrap()).transact()).unwrap()
}

#[test]
fn concurrent_record_edits_use_the_latest_base_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let barrier = Barrier::new(24);
    let ids = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..24)
            .map(|i| {
                let (w, barrier) = (&w, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    let id = create(w, None, "");
                    w.edit_page(PageEdit::Rename {
                        page: id.clone(),
                        title: format!("page {i}"),
                    })
                    .unwrap();
                    id
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<BTreeSet<_>>()
    });
    let before = records(&w);
    assert_eq!(before.len(), 24);
    assert_eq!(ids, before.keys().cloned().collect());
    assert_eq!(
        before
            .values()
            .map(|r| r.placement.order)
            .collect::<BTreeSet<_>>()
            .len(),
        24
    );
    assert_eq!(
        before
            .values()
            .map(|r| r.title.clone())
            .collect::<BTreeSet<_>>(),
        (0..24).map(|i| format!("page {i}")).collect()
    );
    assert_eq!(w.flush(Instant::now() + Duration::from_secs(5)), []);
    drop(w);
    assert_eq!(records(&empty(dir.path())), before);
}

#[test]
fn trash_restore_preserves_tree_sibling_order_and_favorites_field_by_field() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let a = create(&w, None, "A");
    let child = create(&w, Some(a.clone()), "child");
    create(&w, None, "B");
    let mut editor = Editor::new(123);
    editor.apply(&DocId::workspace(), &w.load(&DocId::workspace()).unwrap());
    let mut favorite = records(&w)[&child].clone();
    favorite.favorite = Some(crate::workspace::records::Favorite { order: 7 });
    let update = editor.edit(&DocId::workspace(), |txn| put_page(txn, &child, &favorite));
    w.append(&DocId::workspace(), &update).unwrap();
    let before = records(&w);
    let batch = w
        .edit_page(PageEdit::Trash { page: a })
        .unwrap()
        .trash_batch()
        .unwrap();
    assert_eq!(
        w.tree()
            .unwrap()
            .pages
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        ["B"]
    );
    w.edit_page(PageEdit::Restore { batch }).unwrap();
    let mut after = records(&w);
    for (id, record) in &mut after {
        record.updated_at = before[id].updated_at;
    }
    assert_eq!(after, before);
    assert_eq!(w.tree().unwrap().favorites, [child]);
}

#[test]
fn moves_refuse_cycles_and_restore_uses_top_level_when_original_parent_is_inactive() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let a = create(&w, None, "A");
    let child = create(&w, Some(a.clone()), "child");
    let before = records(&w);
    for parent in [a.clone(), child.clone()] {
        assert!(matches!(
            w.edit_page(PageEdit::Move {
                page: a.clone(),
                parent: Some(parent),
                position: 0
            }),
            Err(PageEditError::Cycle)
        ));
        assert_eq!(records(&w), before);
    }
    let batch = w
        .edit_page(PageEdit::Trash {
            page: child.clone(),
        })
        .unwrap()
        .trash_batch()
        .unwrap();
    w.edit_page(PageEdit::Trash { page: a }).unwrap();
    let last = create(&w, None, "last");
    w.edit_page(PageEdit::Restore { batch }).unwrap();
    let r = records(&w);
    assert_eq!(r[&child].placement.parent, None);
    assert!(r[&child].placement.order > r[&last].placement.order);
}

#[test]
fn edits_preserve_unknown_keys_and_coupled_object_fields() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let id = create(&w, None, "A");
    let parent = create(&w, None, "parent");
    let mut editor = Editor::new(124);
    editor.apply(&DocId::workspace(), &w.load(&DocId::workspace()).unwrap());
    let update = editor.edit(&DocId::workspace(), |txn| {
        let root = txn.get_or_insert_map("pages");
        let yrs::Out::YMap(record) = root.get(txn, id.as_str()).unwrap() else {
            panic!()
        };
        record.insert(txn, "future", "kept");
        let yrs::Out::Any(Any::Map(placement)) = record.get(txn, "placement").unwrap() else {
            panic!()
        };
        let mut placement = (*placement).clone();
        placement.insert("future".into(), Any::from("also kept"));
        record.insert(txn, "placement", Any::from(placement));
    });
    w.append(&DocId::workspace(), &update).unwrap();
    w.edit_page(PageEdit::Rename {
        page: id.clone(),
        title: "renamed".into(),
    })
    .unwrap();
    w.edit_page(PageEdit::Move {
        page: id.clone(),
        parent: Some(parent.clone()),
        position: 0,
    })
    .unwrap();
    let doc = doc_of(&w.load(&DocId::workspace()).unwrap());
    let txn = doc.transact();
    let root = txn.get_map("pages").unwrap();
    let yrs::Out::YMap(record) = root.get(&txn, id.as_str()).unwrap() else {
        panic!()
    };
    assert_eq!(
        record.get(&txn, "future"),
        Some(yrs::Out::Any(Any::from("kept")))
    );
    let yrs::Out::Any(Any::Map(placement)) = record.get(&txn, "placement").unwrap() else {
        panic!()
    };
    assert_eq!(placement.get("future"), Some(&Any::from("also kept")));
    assert_eq!(records(&w)[&id].title, "renamed");
    assert_eq!(records(&w)[&id].placement.parent, Some(parent));
}

#[test]
fn index_search_finds_a_title_renamed_through_the_production_writer() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let id = create(&w, None, "Before");
    w.edit_page(PageEdit::Rename {
        page: id.clone(),
        title: "kestrelword".into(),
    })
    .unwrap();
    assert_eq!(w.tree().unwrap().pages[0].title, "kestrelword");
    assert!(w.index().shared().settle(Duration::from_secs(10)));
    let crate::index::Reply::Ready { value, .. } = w
        .search(&crate::index::SearchQuery {
            text: "kestrelword",
            limit: 5,
            scope: crate::index::SearchScope::All,
        })
        .unwrap()
    else {
        panic!("indexing")
    };
    assert_eq!(
        value.iter().map(|h| h.page_id.clone()).collect::<Vec<_>>(),
        [id]
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn random_create_move_trash_restore_keeps_an_ordered_tree(ops in prop::collection::vec((0u8..4, any::<usize>(), any::<usize>()), 1..100)) {
        let dir = tempfile::tempdir().unwrap();
        let w = empty(dir.path());
        for (op, a, b) in ops {
            let before = records(&w);
            let ids: Vec<_> = before.keys().cloned().collect();
            let active: Vec<_> = w.tree().unwrap().pages.iter().map(|p| p.id.clone()).collect();
            match op {
                0 => { let parent = active.get(b % (active.len() + 1)).cloned(); create(&w, parent, "page"); }
                1 if !active.is_empty() => {
                    let id = active[a % active.len()].clone();
                    let parent = active.get(b % (active.len() + 1)).cloned();
                    let result = w.edit_page(PageEdit::Move { page: id, parent, position: a % (active.len() + 1) });
                    prop_assert!(matches!(result, Ok(_) | Err(PageEditError::Cycle)));
                }
                2 if !active.is_empty() => { w.edit_page(PageEdit::Trash { page: active[a % active.len()].clone() }).unwrap(); }
                3 if !ids.is_empty() => {
                    if let Some(trash) = &before[&ids[a % ids.len()]].trash {
                        w.edit_page(PageEdit::Restore { batch: trash.batch_id.clone() }).unwrap();
                    }
                }
                _ => {}
            }
            let after = records(&w);
            let tree = w.tree().unwrap();
            let mut seen = BTreeSet::new();
            let mut orders = BTreeSet::new();
            for p in &tree.pages {
                prop_assert_eq!(&p.parent, &after[&p.id].placement.parent);
                if let Some(parent) = &p.parent { prop_assert!(seen.contains(parent)); }
                prop_assert!(seen.insert(p.id.clone()));
                prop_assert!(orders.insert((p.parent.clone(), after[&p.id].placement.order)));
            }
            for (id, record) in &after {
                if let Some(trash) = &record.trash {
                    prop_assert_eq!(&record.placement.parent, &trash.original_parent);
                    prop_assert_eq!(record.placement.order, trash.original_order);
                    if let Some(old) = before.get(id).and_then(|r| r.trash.as_ref()) {
                        prop_assert_eq!(trash, old);
                    }
                } else { prop_assert!(seen.contains(id)); }
            }
        }
    }
}

#[test]
fn pending_independent_record_fields_win_after_rename_in_both_client_orders() {
    for client in [1, (1 << 53) - 1] {
        for (key, value) in [
            (
                "favorite",
                Any::from(std::collections::HashMap::from([(
                    "order".into(),
                    Any::from(7),
                )])),
            ),
            (
                "placement",
                Any::from(std::collections::HashMap::from([
                    ("parentId".into(), Any::Null),
                    ("order".into(), Any::from(99)),
                ])),
            ),
            (
                "trash",
                Any::from(std::collections::HashMap::from([
                    ("originalParentId".into(), Any::Null),
                    ("originalOrder".into(), Any::from(99)),
                    ("batchId".into(), Any::from("foreign-batch")),
                    ("trashedAt".into(), Any::from(50)),
                ])),
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut w = empty(dir.path());
            w.record_client = yrs::ClientID::new(100);
            let page = create(&w, None, "Before");
            let id = DocId::workspace();
            let mut foreign = Editor::new(client);
            foreign.apply(&id, &w.load(&id).unwrap());
            let dependency = foreign.edit(&id, |txn| {
                let root = txn.get_or_insert_map("pages");
                let yrs::Out::YMap(record) = root.get(txn, page.as_str()).unwrap() else {
                    panic!()
                };
                let yrs::Out::Any(baseline) = record.get(txn, key).unwrap() else {
                    panic!()
                };
                record.insert(txn, key, baseline);
            });
            let pending = foreign.edit(&id, |txn| {
                let root = txn.get_or_insert_map("pages");
                let yrs::Out::YMap(record) = root.get(txn, page.as_str()).unwrap() else {
                    panic!()
                };
                record.insert(txn, key, value.clone());
            });
            w.append(&id, &pending).unwrap();
            assert_eq!(w.flush(Instant::now() + Duration::from_secs(5)), []);
            w.edit_page(PageEdit::Rename {
                page: page.clone(),
                title: "After".into(),
            })
            .unwrap_or_else(|e| panic!("{key}, client {client}: {e}; {:?}", records(&w)));
            assert_eq!(w.flush(Instant::now() + Duration::from_secs(5)), []);
            drop(w);
            let w = empty(dir.path());
            w.append(&id, &dependency).unwrap();
            assert_eq!(records(&w)[&page].title, "After");
            let doc = doc_of(&w.load(&id).unwrap());
            let txn = doc.transact();
            let root = txn.get_map("pages").unwrap();
            let yrs::Out::YMap(record) = root.get(&txn, page.as_str()).unwrap() else {
                panic!()
            };
            assert_eq!(
                record.get(&txn, key),
                Some(yrs::Out::Any(value)),
                "{key}, client {client}"
            );
        }
    }
}

fn crafted(w: &Workspace, pages: &[(&str, Option<&str>, u64)]) {
    let mut editor = Editor::new(125);
    let update = editor.edit(&DocId::workspace(), |txn| {
        for (id, parent, order) in pages {
            put_page(
                txn,
                &page(id),
                &PageRecord {
                    title: (*id).into(),
                    placement: Placement {
                        parent: parent.map(page),
                        order: *order,
                    },
                    favorite: None,
                    trash: None,
                    created_at: 1,
                    updated_at: 2,
                },
            );
        }
    });
    w.append(&DocId::workspace(), &update).unwrap();
}

fn projected_ids(w: &Workspace) -> Vec<String> {
    w.tree()
        .unwrap()
        .pages
        .iter()
        .map(|p| p.id.to_string())
        .collect()
}

#[test]
fn restoring_tied_siblings_preserves_order_without_writing_the_untouched_sibling() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use yrs::updates::decoder::Decode;
    use yrs::{Observable, Update};

    for subtree in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let w = empty(dir.path());
        let pages = if subtree {
            vec![
                ("p", None, 20),
                ("a", Some("p"), 10),
                ("b", Some("p"), 10),
                ("c", Some("p"), 10),
                ("untouched", None, 30),
            ]
        } else {
            vec![("a", None, 10), ("b", None, 10)]
        };
        crafted(&w, &pages);
        let before = records(&w);
        let expected = if subtree {
            vec!["p", "a", "b", "c", "untouched"]
        } else {
            vec!["a", "b"]
        };
        assert_eq!(projected_ids(&w), expected);
        let observer = doc_of(&w.load(&DocId::workspace()).unwrap());
        let untouched = if subtree { "untouched" } else { "a" };
        let yrs::Out::YMap(record) = observer
            .transact()
            .get_map("pages")
            .unwrap()
            .get(&observer.transact(), untouched)
            .unwrap()
        else {
            panic!()
        };
        let writes = Arc::new(AtomicUsize::new(0));
        let seen = writes.clone();
        record.observe("untouched-writes", move |txn, event| {
            seen.fetch_add(event.keys(txn).len(), Ordering::SeqCst);
        });
        let batch = w
            .edit_page(PageEdit::Trash {
                page: page(if subtree { "p" } else { "b" }),
            })
            .unwrap()
            .trash_batch()
            .unwrap();
        w.edit_page(PageEdit::Restore { batch }).unwrap();
        observer
            .transact_mut()
            .apply_update(Update::decode_v1(&w.load(&DocId::workspace()).unwrap()).unwrap())
            .unwrap();
        assert_eq!(projected_ids(&w), expected);
        let mut after = records(&w);
        assert_eq!(after[&page(untouched)], before[&page(untouched)]);
        for (id, record) in &mut after {
            record.updated_at = before[id].updated_at;
            record.placement.order = before[id].placement.order;
        }
        assert_eq!(after, before);
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn restore_orders_newly_visible_foreign_children_with_the_batch_siblings() {
    for inactive_parent in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let w = empty(dir.path());
        crafted(
            &w,
            &[
                ("t", None, 1),
                ("p", inactive_parent.then_some("t"), 20),
                ("a", Some("p"), 10),
                ("c", Some("p"), 10),
                ("untouched", None, 40),
            ],
        );
        let batch = w
            .edit_page(PageEdit::Trash { page: page("p") })
            .unwrap()
            .trash_batch()
            .unwrap();
        w.edit_page(PageEdit::Trash { page: page("t") }).unwrap();
        let mut foreign = Editor::new(126);
        foreign.apply(&DocId::workspace(), &w.load(&DocId::workspace()).unwrap());
        let update = foreign.edit(&DocId::workspace(), |txn| {
            put_page(
                txn,
                &page("z"),
                &PageRecord {
                    title: "foreign child".into(),
                    placement: Placement {
                        parent: Some(page("p")),
                        order: 10,
                    },
                    favorite: None,
                    trash: None,
                    created_at: 1,
                    updated_at: 2,
                },
            )
        });
        w.append(&DocId::workspace(), &update).unwrap();
        assert_eq!(projected_ids(&w), ["untouched"]);
        let before = records(&w);
        w.edit_page(PageEdit::Restore { batch }).unwrap();
        let expected = if inactive_parent {
            vec!["untouched", "p", "a", "c", "z"]
        } else {
            vec!["p", "a", "c", "z", "untouched"]
        };
        assert_eq!(projected_ids(&w), expected);
        let after = records(&w);
        assert_eq!(after[&page("untouched")], before[&page("untouched")]);
        assert_eq!(
            after[&page("z")].placement,
            Placement {
                parent: Some(page("p")),
                order: 12
            }
        );
        assert_eq!(after[&page("z")].updated_at, 2);
    }
}

#[test]
fn moving_out_of_a_repaired_cycle_keeps_both_pages_roots_and_trashes_only_the_moved_page() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    crafted(&w, &[("a", Some("b"), 10), ("b", Some("a"), 20)]);
    assert_eq!(
        w.tree()
            .unwrap()
            .pages
            .iter()
            .map(|p| (p.id.as_str(), p.parent.as_ref().map(PageId::as_str)))
            .collect::<Vec<_>>(),
        [("a", None), ("b", Some("a"))]
    );
    w.edit_page(PageEdit::Move {
        page: page("b"),
        parent: None,
        position: 1,
    })
    .unwrap();
    assert_eq!(
        w.tree()
            .unwrap()
            .pages
            .iter()
            .map(|p| (p.id.as_str(), p.parent.as_ref().map(PageId::as_str)))
            .collect::<Vec<_>>(),
        [("a", None), ("b", None)]
    );
    assert_eq!(records(&w)[&page("a")].placement.parent, None);
    w.edit_page(PageEdit::Trash { page: page("b") }).unwrap();
    assert_eq!(projected_ids(&w), ["a"]);
    assert_eq!(records(&w)[&page("a")].trash, None);
}

#[test]
fn moving_before_tied_siblings_keeps_every_sibling_in_projected_order() {
    for order in [10, 0] {
        let dir = tempfile::tempdir().unwrap();
        let w = empty(dir.path());
        crafted(
            &w,
            &[("a", None, order), ("b", None, order), ("z", None, 20)],
        );
        w.edit_page(PageEdit::Move {
            page: page("z"),
            parent: None,
            position: 0,
        })
        .unwrap();
        assert_eq!(projected_ids(&w), ["z", "a", "b"]);
        let after = records(&w);
        assert_eq!(after[&page("a")].updated_at, 2);
        assert_eq!(after[&page("b")].updated_at, 2);
    }
}

#[test]
fn end_append_and_collision_shifts_include_projected_orphan_roots() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    crafted(
        &w,
        &[
            ("orphan", Some("missing"), 10 << 20),
            ("root", None, 1 << 20),
        ],
    );
    let new = create(&w, None, "new");
    assert_eq!(projected_ids(&w), ["root", "orphan", new.as_str()]);
    w.edit_page(PageEdit::Move {
        page: new.clone(),
        parent: None,
        position: 1,
    })
    .unwrap();
    assert_eq!(projected_ids(&w), ["root", new.as_str(), "orphan"]);
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    crafted(&w, &[("a", Some("missing"), 0), ("z", None, 20)]);
    w.edit_page(PageEdit::Move {
        page: page("z"),
        parent: None,
        position: 0,
    })
    .unwrap();
    assert_eq!(projected_ids(&w), ["z", "a"]);
    assert_eq!(records(&w)[&page("a")].placement.order, 1);
}

#[test]
fn inserting_into_an_order_gap_and_noop_moves_leave_other_records_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    crafted(
        &w,
        &[
            ("a", None, 1 << 20),
            ("b", None, 2 << 20),
            ("z", None, 3 << 20),
        ],
    );
    let before = records(&w);
    w.edit_page(PageEdit::Move {
        page: page("z"),
        parent: None,
        position: 1,
    })
    .unwrap();
    assert_eq!(projected_ids(&w), ["a", "z", "b"]);
    let after = records(&w);
    assert_eq!(after[&page("a")], before[&page("a")]);
    assert_eq!(after[&page("b")], before[&page("b")]);
    w.edit_page(PageEdit::Move {
        page: page("z"),
        parent: None,
        position: 1,
    })
    .unwrap();
    assert_eq!(records(&w), after);
}

#[test]
fn trash_restore_round_trips_self_parent_and_two_page_cycles_field_by_field() {
    for pages in [
        vec![("a", Some("a"), 10)],
        vec![("a", Some("b"), 10), ("b", Some("a"), 20)],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let w = empty(dir.path());
        crafted(&w, &pages);
        let before_tree = w.tree().unwrap();
        let mut expected = records(&w);
        expected.get_mut(&page("a")).unwrap().placement.parent = None;
        let batch = w
            .edit_page(PageEdit::Trash { page: page("a") })
            .unwrap()
            .trash_batch()
            .unwrap();
        assert_eq!(projected_ids(&w), Vec::<String>::new());
        w.edit_page(PageEdit::Restore { batch }).unwrap();
        let mut after = records(&w);
        for (id, record) in &mut after {
            record.updated_at = expected[id].updated_at;
        }
        assert_eq!(after, expected);
        let mut after_tree = (*w.tree().unwrap()).clone();
        for (actual, before) in after_tree.pages.iter_mut().zip(&before_tree.pages) {
            actual.updated_at = before.updated_at;
        }
        assert_eq!(after_tree, *before_tree);
        assert_eq!(after_tree.pages[0].parent, None);
    }
}

#[test]
fn record_edits_add_one_client_per_launch_and_resume_its_clock() {
    let dir = tempfile::tempdir().unwrap();
    let w = empty(dir.path());
    let id = create(&w, None, "Before");
    for i in 0..20 {
        w.edit_page(PageEdit::Rename {
            page: id.clone(),
            title: format!("title {i}"),
        })
        .unwrap();
    }
    let first = doc_of(&w.load(&DocId::workspace()).unwrap())
        .transact()
        .state_vector();
    assert_eq!(first.len(), 1);
    assert_eq!(w.flush(Instant::now() + Duration::from_secs(5)), []);
    drop(w);
    let w = empty(dir.path());
    for i in 0..20 {
        w.edit_page(PageEdit::Rename {
            page: id.clone(),
            title: format!("reopened {i}"),
        })
        .unwrap();
    }
    let second = doc_of(&w.load(&DocId::workspace()).unwrap())
        .transact()
        .state_vector();
    assert_eq!(second.len(), 2);
    for (client, clock) in first.iter() {
        assert_eq!(second.get(client), *clock);
    }
    assert_eq!(records(&w)[&id].title, "reopened 19");
}
