//! Replay the shared Python cases through Rust services over the installed DDL.
//! HTTP routes, default inputs, and field names below belong to the fixture adapter.

use chrono::NaiveDate;
use finance_desktop_lib::db::{Database, migrations::migrate};
use finance_desktop_lib::domain::Finance;
use finance_desktop_lib::error::{AppError, ErrorCategory};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

type Saved = BTreeMap<String, Value>;

#[test]
fn python_golden_cases() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../fixtures/parity/v1/cases.json")).unwrap();
    assert_eq!(fixtures["version"], 1);
    for case in fixtures["cases"].as_array().unwrap() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("finance.sqlite")).unwrap();
        migrate(&db).unwrap();
        let finance = Finance::new(&db);
        let mut saved = Saved::new();
        let mut write_keys = BTreeMap::new();
        for step in case["steps"].as_array().unwrap() {
            let name = format!("{}/{}", case["id"], step["id"]);
            let path = resolve(&step["path"], &saved);
            let mut body = resolve(&step["body"], &saved);
            let key = body["idempotency_key"]
                .as_str()
                .map_or_else(Uuid::new_v4, |key| {
                    *write_keys
                        .entry(key.to_owned())
                        .or_insert_with(Uuid::new_v4)
                });
            if let Some(body) = body.as_object_mut() {
                body.remove("idempotency_key");
            }
            let result = dispatch(
                &finance,
                step["method"].as_str().unwrap(),
                path.as_str().unwrap(),
                body,
                key,
            );
            let status = step["status"].as_u64().unwrap();
            if status >= 400 {
                let err = result.expect_err(&name);
                let expected = match status {
                    400 | 422 => ErrorCategory::Validation,
                    404 => ErrorCategory::NotFound,
                    409 => ErrorCategory::Conflict,
                    _ => panic!("unknown fixture error status {status}"),
                };
                assert_eq!(err.category, expected, "{name}: {err}");
                continue;
            }
            let result = result.unwrap_or_else(|err| panic!("{name}: {err}"));
            for (field, expected) in step["expect"].as_object().into_iter().flatten() {
                let mut expected = convert_keys(resolve(expected, &saved), camel_case);
                if field == "postings" {
                    expected
                        .as_array_mut()
                        .unwrap()
                        .sort_by_key(|posting| posting["accountId"].as_str().unwrap().to_owned());
                }
                assert_eq!(at(&result, field), &expected, "{name}.{field}");
            }
            for assertion in step["expect_row"].as_array().into_iter().flatten() {
                let filter = convert_keys(resolve(&assertion["where"], &saved), camel_case);
                let rows = at(&result, assertion["collection"].as_str().unwrap())
                    .as_array()
                    .unwrap();
                let matching: Vec<_> = rows
                    .iter()
                    .filter(|item| {
                        filter
                            .as_object()
                            .unwrap()
                            .iter()
                            .all(|(key, value)| item[key] == *value)
                    })
                    .collect();
                assert_eq!(matching.len(), 1, "{name}");
                assert_eq!(
                    at(matching[0], assertion["field"].as_str().unwrap()),
                    &assertion["equals"],
                    "{name}"
                );
            }
            saved.insert(step["id"].as_str().unwrap().into(), result);
        }
    }
}

fn dispatch(
    finance: &Finance<'_>,
    method: &str,
    path: &str,
    mut body: Value,
    request: Uuid,
) -> Result<Value, AppError> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let query: BTreeMap<_, _> = query
        .split('&')
        .filter_map(|part| part.split_once('='))
        .collect();
    let date = |name| query[name].parse::<NaiveDate>().unwrap();
    let id = |value: &str| Uuid::parse_str(value).unwrap();
    let segments: Vec<_> = path.trim_start_matches('/').split('/').collect();
    macro_rules! write {
        ($operation:expr) => {
            Ok(encode($operation?.result))
        };
    }
    match (method, segments.as_slice()) {
        ("POST", ["accounts"]) => write!(finance.create_account(
            request,
            body["name"].as_str().unwrap(),
            decode(body["kind"].clone())
        )),
        ("PATCH", ["accounts", account]) => write!(finance.archive_account(
            request,
            id(account),
            body["archived"].as_bool().unwrap()
        )),
        ("PUT", ["accounts", account, "opening-balance"]) => {
            body["account_id"] = json!(account);
            if body["reason"].is_null() {
                body["reason"] = json!("Set opening");
            }
            write!(finance.set_opening(request, decode(body)))
        }
        ("GET", ["accounts", account, "opening-revisions"]) => {
            Ok(encode(finance.opening_revisions(id(account))?))
        }
        ("GET", ["accounts", account, "balance"]) => Ok(encode(
            finance.account_balance(id(account), date("as_of_date"))?,
        )),
        ("POST", ["categories"]) => write!(finance.create_category(
            request,
            body["name"].as_str().unwrap(),
            decode(body["type"].clone()),
            decode(body["parent_id"].clone())
        )),
        ("PUT", ["categories", category_id]) => {
            let category = finance
                .list_categories(true)?
                .into_iter()
                .find(|item| item.id == id(category_id))
                .unwrap();
            let fields = body.as_object().unwrap();
            let parent = fields
                .get("parent_id")
                .map_or(category.parent_id, |field| decode(field.clone()));
            let kind = fields
                .get("type")
                .map_or(category.kind, |field| decode(field.clone()));
            write!(finance.update_category(
                request,
                id(category_id),
                body["name"].as_str().unwrap_or(&category.name),
                kind,
                parent
            ))
        }
        ("DELETE", ["categories", category]) => {
            write!(finance.delete_category(request, id(category)))
        }
        ("POST", ["categories", category, "archive"]) => {
            write!(finance.archive_category(request, id(category), true))
        }
        ("POST", ["categories", category, "restore"]) => {
            write!(finance.archive_category(request, id(category), false))
        }
        ("GET", ["categories", "tree"]) => Ok(encode(
            finance.category_tree(query.get("include_archived") == Some(&"true"))?,
        )),
        ("GET", ["categories"]) => {
            let categories =
                finance.list_categories(query.get("include_archived") == Some(&"true"))?;
            Ok(json!({"total": categories.len(), "items": categories}))
        }
        ("POST", ["financial-events"]) => write!(finance.post_event(request, decode(body))),
        ("POST", ["financial-events", event, "reverse"]) => {
            write!(finance.reverse_event(request, id(event), "Correction"))
        }
        ("GET", ["financial-events"]) => Ok(encode(finance.list_events(
            query.get("account_id").map(|value| id(value)),
            1,
            100,
        )?)),
        ("POST", ["transactions"]) => {
            body["account_type"] = json!(body["account_type"].as_str().unwrap().to_uppercase());
            Ok(json!({"id": finance.save_legacy_transaction(request, None, decode(body))?.result}))
        }
        ("POST", ["ledger-cutovers", "preview"]) => {
            Ok(encode(finance.preview_cutover(&decode(body))?))
        }
        ("POST", ["ledger-cutovers", "confirm"]) => {
            write!(finance.confirm_cutover(request, decode(body)))
        }
        ("PUT", ["cards", card, "settings"]) => {
            body["card_id"] = json!(card);
            write!(finance.set_card_settings(request, decode(body)))
        }
        ("POST", ["cards", card, "schedules"]) => {
            body["card_id"] = json!(card);
            if body["source"].is_null() {
                body["source"] = json!("estimate");
            }
            write!(finance.create_schedule(request, decode(body)))
        }
        ("POST", ["cards", _, "schedules", schedule, "reserve"]) => {
            write!(finance.fund_card_reserve(request, id(schedule), decode(body)))
        }
        ("POST", ["cards", _, "schedules", schedule, "payments"]) => {
            write!(finance.apply_card_event(
                request,
                id(schedule),
                decode(body["repayment_event_id"].clone()),
                body["amount"].as_i64().unwrap()
            ))
        }
        ("POST", ["investments", account, "valuations"]) => {
            body["account_id"] = json!(account);
            write!(finance.record_valuation(request, decode(body)))
        }
        ("POST", ["investments", account, "holdings-changes"]) => {
            body["account_id"] = json!(account);
            body["reason"] = body["note"].take();
            Ok(json!({"id": finance.mark_holdings_change(request, decode(body))?.result}))
        }
        ("POST", ["reconciliations"]) => {
            write!(finance.create_reconciliation(request, decode(body)))
        }
        ("POST", ["reconciliations", session, "confirm"]) => {
            write!(finance.confirm_reconciliation(request, id(session)))
        }
        ("POST", ["expected-cash-flows"]) => {
            write!(finance.create_expected_flow(request, decode(body)))
        }
        ("GET", ["reports", "canonical-monthly"]) => {
            Ok(encode(finance.canonical_monthly(None, None)?))
        }
        ("GET", ["reports", "monthly"]) => {
            let mut result = encode(finance.legacy_monthly()?);
            for row in result.as_array_mut().unwrap() {
                row["expense"] = row["expenses"].take();
            }
            Ok(result)
        }
        ("GET", ["reports", "cash-forecast"]) => Ok(encode(finance.cash_forecast(
            date("from"),
            date("to"),
            query.get("scenario") == Some(&"true"),
        )?)),
        ("GET", ["reports", "available-cash"]) => Ok(encode(
            finance.available_cash(date("as_of_date"), date("horizon"))?,
        )),
        ("GET", ["reports", "financial-position"]) => {
            Ok(encode(finance.financial_position(date("as_of_date"))?))
        }
        _ => panic!("Unsupported fixture operation {method} {path}"),
    }
}

fn encode(value: impl Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

fn decode<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(convert_keys(value, camel_case)).unwrap()
}

fn camel_case(field: &str) -> String {
    let mut parts = field.split('_');
    let mut name = parts.next().unwrap_or("").to_owned();
    for part in parts {
        let mut chars = part.chars();
        name.extend(chars.next().into_iter().flat_map(char::to_uppercase));
        name.extend(chars);
    }
    name
}

fn convert_keys(value: Value, convert: fn(&str) -> String) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (convert(&key), convert_keys(value, convert)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| convert_keys(item, convert))
                .collect(),
        ),
        other => other,
    }
}

fn at<'a>(mut value: &'a Value, field: &str) -> &'a Value {
    for piece in field.split('.').filter(|piece| !piece.is_empty()) {
        value = if let Ok(index) = piece.parse::<usize>() {
            value.get(index)
        } else {
            value.get(camel_case(piece))
        }
        .unwrap_or_else(|| panic!("Missing field {field} in {value}"));
    }
    value
}

fn resolve(value: &Value, saved: &Saved) -> Value {
    match value {
        Value::String(value) => {
            let mut output = String::new();
            let mut remaining = value.as_str();
            while let Some((prefix, reference)) = remaining.split_once('$') {
                output.push_str(prefix);
                let end = reference
                    .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                    .unwrap_or(reference.len());
                let (reference, tail) = reference.split_at(end);
                let (step, field) = reference.split_once('.').unwrap_or((reference, ""));
                let selected = at(&saved[step], field);
                if output.is_empty() && tail.is_empty() {
                    return selected.clone();
                }
                output.push_str(
                    selected
                        .as_str()
                        .expect("reference inside text must be textual"),
                );
                remaining = tail;
            }
            output.push_str(remaining);
            json!(output)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| resolve(item, saved)).collect())
        }
        Value::Object(items) => Value::Object(
            items
                .iter()
                .map(|(key, value)| (key.clone(), resolve(value, saved)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
