use super::records::{PAGES, PageId, PageRecord, key};
use std::collections::HashMap;
use yrs::{Any, Map, MapPrelim, MapRef, Out, TransactionMut, WriteTxn};

fn pages(txn: &mut TransactionMut) -> MapRef {
    txn.get_or_insert_map(PAGES)
}

/// Writes changed typed fields, keeping every unknown key and coupled object field.
pub(crate) fn put_page(
    txn: &mut TransactionMut,
    id: &PageId,
    prior: Option<&PageRecord>,
    record: &PageRecord,
) {
    let pages = pages(txn);
    let map = match pages.get(txn, id.as_str()) {
        Some(Out::YMap(map)) => map,
        _ => pages.insert(txn, id.as_str(), MapPrelim::default()),
    };
    if prior.is_none_or(|p| p.title != record.title) {
        map.insert(txn, key::TITLE, record.title.as_str());
    }
    if prior.is_none_or(|p| p.placement != record.placement) {
        let placement = merge(
            &map,
            txn,
            key::PLACEMENT,
            [
                (
                    key::PARENT_ID,
                    page_or_null(record.placement.parent.as_ref()),
                ),
                (key::ORDER, number(record.placement.order)),
            ],
        );
        map.insert(txn, key::PLACEMENT, placement);
    }
    if prior.is_none_or(|p| p.favorite != record.favorite) {
        let favorite = match &record.favorite {
            None => Any::Null,
            Some(favorite) => merge(
                &map,
                txn,
                key::FAVORITE,
                [(key::ORDER, number(favorite.order))],
            ),
        };
        map.insert(txn, key::FAVORITE, favorite);
    }
    if prior.is_none_or(|p| p.trash != record.trash) {
        let trash = match &record.trash {
            None => Any::Null,
            Some(trash) => merge(
                &map,
                txn,
                key::TRASH,
                [
                    (
                        key::ORIGINAL_PARENT_ID,
                        page_or_null(trash.original_parent.as_ref()),
                    ),
                    (key::ORIGINAL_ORDER, number(trash.original_order)),
                    (key::BATCH_ID, Any::from(trash.batch_id.as_str())),
                    (key::TRASHED_AT, number(trash.trashed_at)),
                ],
            ),
        };
        map.insert(txn, key::TRASH, trash);
    }
    if prior.is_none_or(|p| p.created_at != record.created_at) {
        map.insert(txn, key::CREATED_AT, number(record.created_at));
    }
    if prior.is_none_or(|p| p.updated_at != record.updated_at) {
        map.insert(txn, key::UPDATED_AT, number(record.updated_at));
    }
}

fn merge<const N: usize>(
    map: &MapRef,
    txn: &TransactionMut,
    name: &str,
    fields: [(&str, Any); N],
) -> Any {
    let mut object: HashMap<String, Any> = match map.get(txn, name) {
        Some(Out::Any(Any::Map(old))) => (*old).clone(),
        _ => HashMap::new(),
    };
    for (field, value) in fields {
        object.insert(field.to_owned(), value);
    }
    Any::from(object)
}

fn number(n: u64) -> Any {
    Any::Number(yrs::any::Number::Int(n as i64))
}

fn page_or_null(page: Option<&PageId>) -> Any {
    page.map_or(Any::Null, |p| Any::from(p.as_str()))
}
