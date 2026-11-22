use super::*;
use crate::commands::wire::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, page};
use rusqlite::Row;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

stored_enum!(AccountKind {
    Cash => "CASH", Bank => "BANK", CreditCard => "CREDIT_CARD", BrokerageCash => "BROKERAGE_CASH",
});
stored_enum!(CategoryType { Income => "income", Expense => "expense" });
stored_enum!(EventKind {
    Income => "INCOME", Expense => "EXPENSE", Transfer => "TRANSFER",
    CardRepayment => "CARD_REPAYMENT", Refund => "REFUND", BalanceAdjustment => "BALANCE_ADJUSTMENT",
});
stored_enum!(DateBasis {
    Known => "known", ConfirmedPurchaseDate => "confirmed_purchase_date", LegacyRecordedDate => "legacy_recorded_date",
});
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BalanceStatus {
    Unavailable,
    IncompleteLegacyMapping,
    Complete,
}

impl AccountKind {
    pub fn liquid(self) -> bool {
        self != Self::CreditCard
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub kind: AccountKind,
    pub currency: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: CategoryType,
    pub parent_id: Option<Uuid>,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryNode {
    #[serde(flatten)]
    pub category: Category,
    pub children: Vec<CategoryNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
    pub account_id: Uuid,
    pub amount: i64,
    pub as_of_date: NaiveDate,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpeningRevision {
    #[serde(flatten)]
    pub opening: Opening,
    pub id: Uuid,
    pub recorded_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpeningInput {
    pub account_id: Uuid,
    pub amount: i64,
    pub as_of_date: NaiveDate,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub account_id: Uuid,
    pub as_of_date: NaiveDate,
    pub balance: Option<i64>,
    pub amount_owed: Option<i64>,
    pub credit_balance: Option<i64>,
    pub status: BalanceStatus,
    pub opening_as_of_date: Option<NaiveDate>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Posting {
    pub account_id: Uuid,
    pub delta: i64,
    pub effective_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: Uuid,
    pub kind: EventKind,
    pub amount: i64,
    pub category_id: Option<Uuid>,
    pub description: Option<String>,
    pub occurred_on: Option<NaiveDate>,
    pub date_basis: DateBasis,
    pub recorded_at: String,
    pub original_event_id: Option<Uuid>,
    pub reverses_event_id: Option<Uuid>,
    pub replaces_event_id: Option<Uuid>,
    pub is_reversal: bool,
    pub postings: Vec<Posting>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub kind: EventKind,
    pub amount: i64,
    pub account_id: Uuid,
    pub destination_account_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub description: Option<String>,
    pub occurred_on: NaiveDate,
    pub effective_date: NaiveDate,
    pub original_event_id: Option<Uuid>,
    pub adjustment_delta: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyPoint {
    pub month: NaiveDate,
    pub income: i64,
    pub gross_spending: i64,
    pub refunds: i64,
    pub net_spending: i64,
    pub date_basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCategoryTotal {
    pub category_id: Uuid,
    pub category_name: String,
    pub gross_spending: i64,
    pub refunds: i64,
    pub net_spending: i64,
    pub date_basis: String,
}

fn account_row(r: &Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: id(r, 0)?,
        name: r.get(1)?,
        kind: AccountKind::read(r, 2)?,
        currency: r.get(3)?,
        archived_at: r.get(4)?,
    })
}

fn category_row(r: &Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: id(r, 0)?,
        name: r.get(1)?,
        kind: CategoryType::read(r, 2)?,
        parent_id: optional_id(r, 3)?,
        archived_at: r.get(4)?,
    })
}

pub(super) fn id(r: &Row<'_>, col: usize) -> rusqlite::Result<Uuid> {
    let raw: String = r.get(col)?;
    raw.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(error))
    })
}

pub(super) fn optional_id(r: &Row<'_>, col: usize) -> rusqlite::Result<Option<Uuid>> {
    if matches!(r.get_ref(col)?, rusqlite::types::ValueRef::Null) {
        Ok(None)
    } else {
        id(r, col).map(Some)
    }
}

pub(super) fn key(id: &Uuid) -> String {
    id.to_string()
}

pub(super) fn account(conn: &Connection, account_id: Uuid) -> Result<Account, AppError> {
    Ok(conn.query_row(
        "SELECT id,name,kind,currency,archived_at FROM accounts WHERE id=?1",
        [key(&account_id)],
        account_row,
    )?)
}

pub(super) fn accounts(conn: &Connection, archived: bool) -> Result<Vec<Account>, AppError> {
    rows(
        conn,
        "SELECT id,name,kind,currency,archived_at FROM accounts
                WHERE ?1 OR archived_at IS NULL ORDER BY name,id",
        [archived],
        account_row,
    )
}

pub(super) fn category(conn: &Connection, category_id: Uuid) -> Result<Category, AppError> {
    Ok(conn.query_row(
        "SELECT id,name,type,parent_id,archived_at FROM categories WHERE id=?1",
        [key(&category_id)],
        category_row,
    )?)
}

pub(super) fn active(account: &Account) -> Result<(), AppError> {
    if account.archived_at.is_some() {
        return Err(conflict(
            codes::ACCOUNT_ARCHIVED,
            "This account has been archived.",
        ));
    }
    Ok(())
}

pub(super) fn latest_opening(
    conn: &Connection,
    account_id: Uuid,
) -> Result<Option<Opening>, AppError> {
    Ok(conn
        .query_row(
            "SELECT amount,as_of_date,revision FROM account_opening_revisions
         WHERE account_id=?1 ORDER BY revision DESC LIMIT 1",
            [key(&account_id)],
            |r| {
                Ok(Opening {
                    account_id,
                    amount: r.get(0)?,
                    as_of_date: r.get(1)?,
                    revision: r.get(2)?,
                })
            },
        )
        .optional()?)
}

pub(super) fn cutover_date(conn: &Connection) -> Result<Option<NaiveDate>, AppError> {
    Ok(conn
        .query_row(
            "SELECT as_of_date FROM ledger_cutover WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?)
}

pub(super) fn add_opening(
    conn: &Connection,
    input: &OpeningInput,
    cutover: bool,
) -> Result<Opening, AppError> {
    let account = account(conn, input.account_id)?;
    active(&account)?;
    date(input.as_of_date)?;
    yen(input.amount, "amount")?;
    let reason = label(&input.reason, "reason", MAX_DESCRIPTION_CHARS)?;
    let current = latest_opening(conn, account.id)?;
    if !cutover && cutover_date(conn)?.is_some_and(|boundary| input.as_of_date < boundary) {
        return Err(invalid("asOfDate", "Opening cannot precede the cutover."));
    }
    if exists(
        conn,
        "SELECT 1 FROM account_postings WHERE account_id=?1 AND effective_date<?2",
        params![key(&account.id), input.as_of_date],
    )? {
        return Err(blocked("The opening would exclude existing postings."));
    }
    if account.kind == AccountKind::CreditCard
        && exists(
            conn,
            "SELECT 1 FROM card_schedules WHERE card_id=?1 AND purchase_event_id IS NULL
         GROUP BY card_id HAVING SUM(principal_due)>?2",
            params![key(&account.id), (-input.amount).max(0)],
        )?
    {
        return Err(blocked(
            "The opening would leave scheduled card debt without principal.",
        ));
    }
    super::review::guard_confirmed(
        conn,
        account.id,
        input
            .as_of_date
            .min(current.as_ref().map_or(input.as_of_date, |o| o.as_of_date)),
    )?;
    let revision = current.map_or(1, |o| o.revision + 1);
    conn.execute(
        "INSERT INTO account_opening_revisions(id,account_id,amount,as_of_date,revision,reason,recorded_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![key(&Uuid::new_v4()), key(&account.id), input.amount, input.as_of_date, revision, reason, now()],
    )?;
    Ok(Opening {
        account_id: account.id,
        amount: input.amount,
        as_of_date: input.as_of_date,
        revision,
    })
}

/// Resolve ledger-wide coverage once per database snapshot.
pub(super) fn mapping_complete(conn: &Connection) -> Result<bool, AppError> {
    exists(
        conn,
        "SELECT 1 FROM ledger_cutover WHERE singleton=1
         AND NOT EXISTS(SELECT 1 FROM legacy_provenance WHERE status='unresolved')",
        [],
    )
}

pub(super) fn balance(
    conn: &Connection,
    account: &Account,
    on: NaiveDate,
    mapping_complete: bool,
) -> Result<Balance, AppError> {
    date(on)?;
    let opening = latest_opening(conn, account.id)?;
    let mut result = Balance {
        account_id: account.id,
        as_of_date: on,
        balance: None,
        amount_owed: None,
        credit_balance: None,
        status: BalanceStatus::Unavailable,
        opening_as_of_date: opening.as_ref().map(|o| o.as_of_date),
        reason: Some("missing_opening".into()),
    };
    let Some(opening) = opening.filter(|o| o.as_of_date <= on) else {
        return Ok(result);
    };
    let delta: i64 = conn.query_row(
        "SELECT COALESCE(SUM(delta),0) FROM account_postings WHERE account_id=?1 AND effective_date BETWEEN ?2 AND ?3",
        params![key(&account.id), opening.as_of_date, on], |r| r.get(0))?;
    let signed = total([opening.amount, delta])?;
    result.balance = Some(signed);
    if account.kind == AccountKind::CreditCard {
        result.amount_owed = Some((-signed).max(0));
        result.credit_balance = Some(signed.max(0));
    }
    result.status = if mapping_complete {
        BalanceStatus::Complete
    } else {
        BalanceStatus::IncompleteLegacyMapping
    };
    result.reason = (!mapping_complete).then(|| "incomplete_legacy_mapping".into());
    Ok(result)
}

fn require_posting_date(
    conn: &Connection,
    account: &Account,
    effective: NaiveDate,
) -> Result<(), AppError> {
    if latest_opening(conn, account.id)?.is_none_or(|opening| effective < opening.as_of_date) {
        return Err(AppError::invalid(
            codes::MISSING_OPENING,
            "effectiveDate",
            "Set an opening on or before the posting date.",
        ));
    }
    if cutover_date(conn)?.is_some_and(|cutover| effective < cutover) {
        return Err(invalid(
            "effectiveDate",
            "Posting date cannot precede confirmed cutover.",
        ));
    }
    Ok(())
}

pub(super) fn event(conn: &Connection, event_id: Uuid) -> Result<Event, AppError> {
    events_with_postings(
        conn,
        "SELECT e.*, e.occurred_on AS sort_date FROM financial_events e WHERE e.id=?1",
        [key(&event_id)],
    )?
    .pop()
    .ok_or_else(missing)
}

fn events_with_postings(
    conn: &Connection,
    selection: &str,
    bindings: impl Params,
) -> Result<Vec<Event>, AppError> {
    // Apply pagination to events before joining their (possibly multiple) postings.
    // The selection is local SQL and all arguments are bound parameters.
    let sql = format!(
        "WITH selected AS ({selection})
         SELECT e.id,e.kind,e.amount,e.category_id,e.description,e.occurred_on,e.date_basis,
                e.recorded_at,e.original_event_id,e.reverses_event_id,e.replaces_event_id,
                e.is_reversal,p.account_id,p.delta,p.effective_date
         FROM selected e LEFT JOIN account_postings p ON p.event_id=e.id
         ORDER BY e.sort_date DESC,e.recorded_at DESC,e.id DESC,p.account_id"
    );
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query(bindings)?;
    let mut events: Vec<Event> = Vec::new();
    while let Some(row) = rows.next()? {
        let event_id = id(row, 0)?;
        if events.last().is_none_or(|event| event.id != event_id) {
            events.push(Event {
                id: event_id,
                kind: EventKind::read(row, 1)?,
                amount: row.get(2)?,
                category_id: optional_id(row, 3)?,
                description: row.get(4)?,
                occurred_on: row.get(5)?,
                date_basis: DateBasis::read(row, 6)?,
                recorded_at: row.get(7)?,
                original_event_id: optional_id(row, 8)?,
                reverses_event_id: optional_id(row, 9)?,
                replaces_event_id: optional_id(row, 10)?,
                is_reversal: row.get(11)?,
                postings: Vec::new(),
            });
        }
        if let Some(account_id) = optional_id(row, 12)? {
            events
                .last_mut()
                .expect("event was read")
                .postings
                .push(Posting {
                    account_id,
                    delta: row.get(13)?,
                    effective_date: row.get(14)?,
                });
        }
    }
    Ok(events)
}

pub(super) fn insert_event(conn: &Connection, value: &Event) -> Result<(), AppError> {
    conn.execute("INSERT INTO financial_events
        (id,kind,amount,category_id,description,occurred_on,date_basis,recorded_at,original_event_id,
         reverses_event_id,replaces_event_id,is_reversal) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![key(&value.id),value.kind.as_str(),value.amount,value.category_id.as_ref().map(key),
            value.description,value.occurred_on,value.date_basis.as_str(),value.recorded_at,
            value.original_event_id.as_ref().map(key), value.reverses_event_id.as_ref().map(key),
            value.replaces_event_id.as_ref().map(key),value.is_reversal])?;
    for posting in &value.postings {
        conn.execute(
            "INSERT INTO account_postings (id,event_id,account_id,delta,effective_date)
            VALUES (?1,?2,?3,?4,?5)",
            params![
                key(&Uuid::new_v4()),
                key(&value.id),
                key(&posting.account_id),
                posting.delta,
                posting.effective_date
            ],
        )?;
    }
    Ok(())
}

pub(super) fn reversed(conn: &Connection, event_id: Uuid) -> Result<bool, AppError> {
    exists(
        conn,
        "SELECT 1 FROM financial_events WHERE reverses_event_id=?1",
        [key(&event_id)],
    )
}

pub(super) fn make_event(
    conn: &Connection,
    input: &EventInput,
    replaces: Option<Uuid>,
) -> Result<Event, AppError> {
    use EventKind::*;
    let value = positive_yen(input.amount, "amount")?;
    date(input.occurred_on)?;
    date(input.effective_date)?;
    if cutover_date(conn)?.is_some_and(|start| input.occurred_on < start) {
        return Err(invalid(
            "occurredOn",
            "Occurrence cannot precede confirmed cutover.",
        ));
    }
    let source = account(conn, input.account_id)?;
    active(&source)?;
    require_posting_date(conn, &source, input.effective_date)?;
    let destination = input
        .destination_account_id
        .map(|target| account(conn, target))
        .transpose()?;
    match (&destination, input.kind) {
        (Some(target), Transfer | CardRepayment) => {
            active(target)?;
            require_posting_date(conn, target, input.effective_date)?;
            if source.id == target.id
                || !source.kind.liquid()
                || (target.kind == AccountKind::CreditCard) != (input.kind == CardRepayment)
            {
                return Err(invalid(
                    "destinationAccountId",
                    "These accounts cannot make this movement.",
                ));
            }
        }
        (None, Transfer | CardRepayment) | (Some(_), _) => {
            return Err(invalid(
                "destinationAccountId",
                "Invalid destination for this event.",
            ));
        }
        (None, _) => {}
    }
    if input.kind == Income && !source.kind.liquid() {
        return Err(invalid("accountId", "Income requires a liquid account."));
    }
    if input.kind != Refund && input.original_event_id.is_some() {
        return Err(invalid(
            "originalEventId",
            "An original purchase applies only to refunds.",
        ));
    }
    let category_id = match input.kind {
        Income | Expense => {
            let category = category(
                conn,
                input
                    .category_id
                    .ok_or_else(|| invalid("categoryId", "Select a matching category."))?,
            )?;
            let expected = if input.kind == Income {
                CategoryType::Income
            } else {
                CategoryType::Expense
            };
            if category.archived_at.is_some() || category.kind != expected {
                return Err(invalid(
                    "categoryId",
                    "Select an active category of the matching type.",
                ));
            }
            Some(category.id)
        }
        Refund => {
            let original = event(
                conn,
                input
                    .original_event_id
                    .ok_or_else(|| invalid("originalEventId", "Select the original purchase."))?,
            )?;
            if original.kind != Expense || original.is_reversal || reversed(conn, original.id)? {
                return Err(blocked("The original purchase cannot be refunded."));
            }
            if input.category_id.is_some() && input.category_id != original.category_id {
                return Err(invalid(
                    "categoryId",
                    "The refund category must match the purchase.",
                ));
            }
            let used = amount(
                conn,
                "SELECT COALESCE(SUM(CASE WHEN is_reversal THEN -amount ELSE amount END),0)
                FROM financial_events WHERE original_event_id=?1 AND kind='REFUND'",
                [key(&original.id)],
            )?;
            if used + value > original.amount {
                return Err(invalid(
                    "amount",
                    "Refund exceeds the remaining purchase amount.",
                ));
            }
            original.category_id
        }
        _ if input.category_id.is_some() => {
            return Err(invalid("categoryId", "This event does not use a category."));
        }
        _ => None,
    };
    let description = text(
        input.description.as_deref(),
        "description",
        MAX_DESCRIPTION_CHARS,
        input.kind == BalanceAdjustment,
    )?;
    let delta = match (input.kind, input.adjustment_delta) {
        (BalanceAdjustment, Some(delta)) if delta.checked_abs() == Some(value) => delta,
        (BalanceAdjustment, _) | (_, Some(_)) => {
            return Err(invalid(
                "adjustmentDelta",
                "A balance adjustment requires a signed amount equal in magnitude.",
            ));
        }
        (Income | Refund, None) => value,
        (_, None) => -value,
    };
    let mut postings = vec![Posting {
        account_id: source.id,
        delta,
        effective_date: input.effective_date,
    }];
    if let Some(dest) = destination {
        postings.push(Posting {
            account_id: dest.id,
            delta: value,
            effective_date: input.effective_date,
        });
    }
    postings.sort_by_key(|p| p.account_id);
    let event = Event {
        id: Uuid::new_v4(),
        kind: input.kind,
        amount: value,
        category_id,
        description,
        occurred_on: Some(input.occurred_on),
        date_basis: DateBasis::Known,
        recorded_at: now(),
        original_event_id: input.original_event_id,
        reverses_event_id: None,
        replaces_event_id: replaces,
        is_reversal: false,
        postings,
    };
    insert_event(conn, &event)?;
    Ok(event)
}

pub(super) fn reverse(conn: &Connection, event_id: Uuid, reason: &str) -> Result<Event, AppError> {
    let original = event(conn, event_id)?;
    if original.is_reversal || reversed(conn, event_id)? {
        return Err(blocked("The event cannot be reversed again."));
    }
    if exists(conn, "SELECT 1 FROM financial_events f WHERE f.original_event_id=?1 AND f.kind='REFUND'
        AND NOT f.is_reversal AND NOT EXISTS (SELECT 1 FROM financial_events r WHERE r.reverses_event_id=f.id)",
        [key(&event_id)])? {
        return Err(blocked("Reverse linked refunds first."));
    }
    if exists(
        conn,
        "SELECT 1 FROM card_schedules WHERE purchase_event_id=?1 UNION ALL
        SELECT 1 FROM card_applications WHERE event_id=?1",
        [key(&event_id)],
    )? {
        return Err(blocked("The event is linked to a card schedule."));
    }
    if exists(
        conn,
        "SELECT 1 FROM flow_matches WHERE event_id=?1",
        [key(&event_id)],
    )? {
        return Err(blocked(
            "Unmatch this event from the forecast before reversing it.",
        ));
    }
    for posting in &original.postings {
        super::review::guard_confirmed(conn, posting.account_id, posting.effective_date)?;
    }
    let value = Event {
        id: Uuid::new_v4(),
        description: Some(label(reason, "reason", MAX_DESCRIPTION_CHARS)?),
        recorded_at: now(),
        reverses_event_id: Some(event_id),
        replaces_event_id: None,
        is_reversal: true,
        postings: original
            .postings
            .into_iter()
            .map(|p| Posting {
                delta: -p.delta,
                ..p
            })
            .collect(),
        ..original
    };
    insert_event(conn, &value)?;
    Ok(value)
}

impl Finance<'_> {
    pub fn list_accounts(&self, archived: bool) -> Result<Vec<Account>, AppError> {
        self.read(|db| accounts(db, archived))
    }

    pub fn create_account(
        &self,
        request: Uuid,
        name: &str,
        kind: AccountKind,
    ) -> Result<WriteResponse<Account>, AppError> {
        let name = label(name, "name", MAX_NAME_CHARS)?;
        self.write(request, "create_account", &(&name, kind), |db| {
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO accounts(id,name,kind,created_at) VALUES (?1,?2,?3,?4)",
                params![key(&id), name, kind.as_str(), now()],
            )?;
            account(db, id)
        })
    }

    pub fn archive_account(
        &self,
        request: Uuid,
        account_id: Uuid,
        archived: bool,
    ) -> Result<WriteResponse<Account>, AppError> {
        self.write(request, "archive_account", &(account_id, archived), |db| {
            account(db, account_id)?;
            let today = profile_today(db)?;
            if archived
                && exists(
                    db,
                    "SELECT 1 FROM subscriptions
                     WHERE account_id=?1
                       AND (status <> 'canceled' OR canceled_effective_on > ?2)",
                    params![key(&account_id), today],
                )?
            {
                return Err(conflict(
                    codes::SUBSCRIPTION_ACTIVE_REFERENCE,
                    "Reassign or cancel subscriptions using this account before archiving it.",
                ));
            }
            db.execute("UPDATE accounts SET archived_at=CASE WHEN ?2 THEN COALESCE(archived_at,?3) END WHERE id=?1",
                params![key(&account_id), archived, now()])?;
            account(db, account_id)
        })
    }

    pub fn list_categories(&self, archived: bool) -> Result<Vec<Category>, AppError> {
        self.read(|db| {
            rows(
                db,
                "SELECT id,name,type,parent_id,archived_at FROM categories
            WHERE ?1 OR archived_at IS NULL ORDER BY created_at,id",
                [archived],
                category_row,
            )
        })
    }

    pub fn category_tree(&self, archived: bool) -> Result<Vec<CategoryNode>, AppError> {
        let categories = self.list_categories(archived)?;
        let visible: BTreeSet<_> = categories.iter().map(|category| category.id).collect();
        let mut children = BTreeMap::<Option<Uuid>, Vec<Category>>::new();
        for category in categories {
            let parent = category.parent_id.filter(|parent| visible.contains(parent));
            children.entry(parent).or_default().push(category);
        }
        fn branch(
            children: &mut BTreeMap<Option<Uuid>, Vec<Category>>,
            parent: Option<Uuid>,
        ) -> Vec<CategoryNode> {
            children
                .remove(&parent)
                .unwrap_or_default()
                .into_iter()
                .map(|category| {
                    let children = branch(children, Some(category.id));
                    CategoryNode { category, children }
                })
                .collect()
        }
        Ok(branch(&mut children, None))
    }

    pub fn create_category(
        &self,
        request: Uuid,
        name: &str,
        kind: CategoryType,
        parent: Option<Uuid>,
    ) -> Result<WriteResponse<Category>, AppError> {
        let name = label(name, "name", MAX_NAME_CHARS)?;
        self.write(request, "create_category", &(&name, kind, parent), |db| {
            validate_parent(db, None, kind, parent)?;
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO categories(id,name,type,parent_id,created_at) VALUES (?1,?2,?3,?4,?5)",
                params![
                    key(&id),
                    name,
                    kind.as_str(),
                    parent.as_ref().map(key),
                    now()
                ],
            )?;
            category(db, id)
        })
    }

    pub fn update_category(
        &self,
        request: Uuid,
        category_id: Uuid,
        name: &str,
        kind: CategoryType,
        parent: Option<Uuid>,
    ) -> Result<WriteResponse<Category>, AppError> {
        let name = label(name, "name", MAX_NAME_CHARS)?;
        self.write(
            request,
            "update_category",
            &(category_id, &name, kind, parent),
            |db| {
                let old = category(db, category_id)?;
                if old.kind != kind && category_used(db, category_id)? {
                    return Err(blocked(
                        "A category with history or children cannot change type.",
                    ));
                }
                validate_parent(db, Some(category_id), kind, parent)?;
                db.execute(
                    "UPDATE categories SET name=?2,type=?3,parent_id=?4 WHERE id=?1",
                    params![
                        key(&category_id),
                        name,
                        kind.as_str(),
                        parent.as_ref().map(key)
                    ],
                )?;
                category(db, category_id)
            },
        )
    }

    pub fn delete_category(
        &self,
        request: Uuid,
        category_id: Uuid,
    ) -> Result<WriteResponse<Uuid>, AppError> {
        self.write(request, "delete_category", &category_id, |db| {
            category(db, category_id)?;
            if category_used(db, category_id)? {
                return Err(blocked("Archive categories with history or children."));
            }
            db.execute("DELETE FROM categories WHERE id=?1", [key(&category_id)])?;
            Ok(category_id)
        })
    }

    pub fn archive_category(
        &self,
        request: Uuid,
        category_id: Uuid,
        archived: bool,
    ) -> Result<WriteResponse<Category>, AppError> {
        self.write(
            request,
            "archive_category",
            &(category_id, archived),
            |db| {
                let old = category(db, category_id)?;
                if archived {
                    let today = profile_today(db)?;
                    if exists(
                        db,
                        "SELECT 1 FROM subscriptions
                         WHERE category_id=?1
                           AND (status <> 'canceled' OR canceled_effective_on > ?2)",
                        params![key(&category_id), today],
                    )? {
                        return Err(conflict(
                            codes::SUBSCRIPTION_ACTIVE_REFERENCE,
                            "Reassign or cancel subscriptions using this category before archiving it.",
                        ));
                    }
                    db.execute(
                        "UPDATE categories SET archived_at=COALESCE(archived_at,?2) WHERE id=?1",
                        params![key(&category_id), now()],
                    )?;
                } else {
                    validate_parent(db, Some(category_id), old.kind, old.parent_id)?;
                    db.execute(
                        "UPDATE categories SET archived_at=NULL WHERE id=?1",
                        [key(&category_id)],
                    )?;
                }
                category(db, category_id)
            },
        )
    }

    pub fn set_opening(
        &self,
        request: Uuid,
        input: OpeningInput,
    ) -> Result<WriteResponse<Opening>, AppError> {
        self.write(request, "set_opening", &input, |db| {
            add_opening(db, &input, false)
        })
    }

    pub fn opening_revisions(&self, account_id: Uuid) -> Result<Vec<OpeningRevision>, AppError> {
        self.read(|db| {
            account(db, account_id)?;
            rows(db, "SELECT id,amount,as_of_date,revision,recorded_at,reason FROM account_opening_revisions
                WHERE account_id=?1 ORDER BY revision", [key(&account_id)], |r| Ok(OpeningRevision {
                opening:Opening { account_id, amount:r.get(1)?, as_of_date:r.get(2)?, revision:r.get(3)? },
                id:id(r,0)?, recorded_at:r.get(4)?, reason:r.get(5)?,
            }))
        })
    }

    pub fn account_balance(&self, account_id: Uuid, on: NaiveDate) -> Result<Balance, AppError> {
        self.read(|db| balance(db, &account(db, account_id)?, on, mapping_complete(db)?))
    }

    pub fn post_event(
        &self,
        request: Uuid,
        input: EventInput,
    ) -> Result<WriteResponse<Event>, AppError> {
        self.write(request, "post_event", &input, |db| {
            make_event(db, &input, None)
        })
    }

    pub fn reverse_event(
        &self,
        request: Uuid,
        id: Uuid,
        reason: &str,
    ) -> Result<WriteResponse<Event>, AppError> {
        self.write(request, "reverse_event", &(id, reason), |db| {
            reverse(db, id, reason)
        })
    }

    pub fn replace_event(
        &self,
        request: Uuid,
        id: Uuid,
        reason: &str,
        replacement: EventInput,
    ) -> Result<WriteResponse<Event>, AppError> {
        self.write(
            request,
            "replace_event",
            &(id, reason, &replacement),
            |db| {
                reverse(db, id, reason)?;
                make_event(db, &replacement, Some(id))
            },
        )
    }

    pub fn financial_event(&self, event_id: Uuid) -> Result<Event, AppError> {
        self.read(|db| event(db, event_id))
    }

    pub fn list_events(
        &self,
        account_id: Option<Uuid>,
        number: u32,
        size: u32,
    ) -> Result<Page<Event>, AppError> {
        let (number, size) = page(number, size)?;
        let offset = (i64::from(number) - 1) * i64::from(size);
        self.read(|db| {
            let (total, items) = if let Some(account_id) = account_id {
                account(db, account_id)?;
                let account_id = key(&account_id);
                let total = db.query_row(
                    "SELECT COUNT(*) FROM account_postings WHERE account_id=?1",
                    [&account_id],
                    |r| r.get(0),
                )?;
                let items = events_with_postings(
                    db,
                    "SELECT e.*, p.effective_date AS sort_date
                     FROM account_postings p JOIN financial_events e ON e.id=p.event_id
                     WHERE p.account_id=?1
                     ORDER BY sort_date DESC,e.recorded_at DESC,e.id DESC LIMIT ?2 OFFSET ?3",
                    params![account_id, size, offset],
                )?;
                (total, items)
            } else {
                let total =
                    db.query_row("SELECT COUNT(*) FROM financial_events", [], |r| r.get(0))?;
                let items = events_with_postings(
                    db,
                    "SELECT e.*, e.occurred_on AS sort_date FROM financial_events e
                     ORDER BY sort_date DESC,e.recorded_at DESC,e.id DESC LIMIT ?1 OFFSET ?2",
                    params![size, offset],
                )?;
                (total, items)
            };
            Ok(Page {
                items,
                total,
                page: number,
                page_size: size,
            })
        })
    }

    pub fn canonical_by_category(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<CanonicalCategoryTotal>, AppError> {
        report_range(from, to)?;
        self.read(|db| {
            let groups=rows(db, "SELECT c.id,c.name,
                SUM(CASE WHEN e.kind='EXPENSE' THEN CASE WHEN e.is_reversal THEN -e.amount ELSE e.amount END ELSE 0 END),
                SUM(CASE WHEN e.kind='REFUND' THEN CASE WHEN e.is_reversal THEN -e.amount ELSE e.amount END ELSE 0 END)
                FROM financial_events e JOIN categories c ON c.id=e.category_id
                WHERE e.occurred_on IS NOT NULL AND e.kind IN ('EXPENSE','REFUND')
                AND (?1 IS NULL OR e.occurred_on>=?1) AND (?2 IS NULL OR e.occurred_on<=?2)
                GROUP BY c.id,c.name ORDER BY c.name,c.id",params![from,to],
                |r|Ok((id(r,0)?,r.get(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?;
            groups.into_iter().map(|(category_id,category_name,gross,refunds)|Ok(CanonicalCategoryTotal {
                category_id,category_name,gross_spending:yen(gross,"grossSpending")?,refunds:yen(refunds,"refunds")?,
                net_spending:total([gross,-refunds])?,date_basis:"known_only".into(),
            })).collect()
        })
    }

    pub fn canonical_monthly(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<MonthlyPoint>, AppError> {
        self.read(|db| monthly(db, from, to))
    }
}

pub(super) fn monthly(
    db: &Connection,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<MonthlyPoint>, AppError> {
    report_range(from, to)?;
    let grouped = rows(db,"SELECT substr(occurred_on,1,7)||'-01',
        SUM(CASE WHEN kind='INCOME' THEN signed ELSE 0 END),
        SUM(CASE WHEN kind='EXPENSE' THEN signed ELSE 0 END),
        SUM(CASE WHEN kind='REFUND' THEN signed ELSE 0 END)
        FROM (SELECT kind,occurred_on,CASE WHEN is_reversal THEN -amount ELSE amount END AS signed
        FROM financial_events WHERE occurred_on IS NOT NULL AND kind IN ('INCOME','EXPENSE','REFUND')
        AND (?1 IS NULL OR occurred_on>=?1) AND (?2 IS NULL OR occurred_on<=?2))
        GROUP BY substr(occurred_on,1,7) ORDER BY 1",params![from,to],|r| {
        Ok((r.get::<_,NaiveDate>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))
    })?;
    grouped
        .into_iter()
        .map(|(month, income, gross, refunds)| {
            Ok(MonthlyPoint {
                month,
                income: yen(income, "income")?,
                gross_spending: yen(gross, "grossSpending")?,
                refunds: yen(refunds, "refunds")?,
                net_spending: total([gross, -refunds])?,
                date_basis: "known_only".into(),
            })
        })
        .collect()
}

pub(super) fn report_range(from: Option<NaiveDate>, to: Option<NaiveDate>) -> Result<(), AppError> {
    for value in [from, to].into_iter().flatten() {
        date(value)?;
    }
    if let (Some(from), Some(to)) = (from, to) {
        range(from, to, 36_600)?;
    }
    Ok(())
}

fn validate_parent(
    db: &Connection,
    child: Option<Uuid>,
    kind: CategoryType,
    mut parent: Option<Uuid>,
) -> Result<(), AppError> {
    let mut direct = true;
    while let Some(parent_id) = parent {
        if Some(parent_id) == child {
            return Err(invalid(
                "parentId",
                "Category hierarchy cannot contain a cycle.",
            ));
        }
        let value = category(db, parent_id)?;
        if value.kind != kind || (direct && value.archived_at.is_some()) {
            return Err(invalid(
                "parentId",
                "Select an active parent of the same type.",
            ));
        }
        parent = value.parent_id;
        direct = false;
    }
    Ok(())
}

fn category_used(db: &Connection, category_id: Uuid) -> Result<bool, AppError> {
    exists(
        db,
        "SELECT 1 FROM categories WHERE parent_id=?1 UNION ALL
        SELECT 1 FROM financial_events WHERE category_id=?1 UNION ALL
        SELECT 1 FROM legacy_transactions WHERE category_id=?1 UNION ALL
        SELECT 1 FROM category_budgets WHERE category_id=?1 UNION ALL
        SELECT 1 FROM expected_flows WHERE budget_category_id=?1 UNION ALL
        SELECT 1 FROM subscriptions WHERE category_id=?1 UNION ALL
        SELECT 1 FROM legacy_provenance WHERE category_id=?1",
        [key(&category_id)],
    )
}
