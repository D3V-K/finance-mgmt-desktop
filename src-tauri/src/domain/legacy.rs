use super::ledger::*;
use super::*;
use crate::commands::wire::MAX_DESCRIPTION_CHARS;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

stored_enum!(LegacyAccount { Cash => "CASH", Bank => "BANK" });

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyTransaction {
    pub amount: i64,
    pub description: Option<String>,
    pub transaction_date: NaiveDate,
    pub category_id: Uuid,
    pub account_type: LegacyAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyTransfer {
    pub amount: i64,
    pub description: Option<String>,
    pub transfer_date: NaiveDate,
    pub from: LegacyAccount,
    pub to: LegacyAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyOpening {
    pub account_type: LegacyAccount,
    pub amount: i64,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CutoverOpening {
    pub account_id: Uuid,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMapping {
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub effective_date: NaiveDate,
    pub occurred_on: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransferMapping {
    pub transfer_id: Uuid,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub effective_date: NaiveDate,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyMappings {
    #[serde(default)]
    pub transactions: Vec<TransactionMapping>,
    #[serde(default)]
    pub transfers: Vec<TransferMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CutoverInput {
    pub as_of_date: NaiveDate,
    pub openings: Vec<CutoverOpening>,
    #[serde(flatten)]
    pub mappings: LegacyMappings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CutoverPreview {
    pub as_of_date: NaiveDate,
    pub legacy_transaction_count: usize,
    pub legacy_transfer_count: usize,
    pub category_sums: BTreeMap<Uuid, i64>,
    pub legacy_date_min: Option<NaiveDate>,
    pub legacy_date_max: Option<NaiveDate>,
    pub unresolved_transaction_ids: Vec<Uuid>,
    pub unresolved_transfer_ids: Vec<Uuid>,
    pub unresolved_count: usize,
    pub mapped_count: usize,
    pub opening_total: i64,
    pub legacy_account_type_is_unverified: bool,
    pub purchase_date_basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Cutover {
    pub id: Uuid,
    pub as_of_date: NaiveDate,
    pub unresolved_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub source_table: String,
    pub source_id: Uuid,
    pub recorded_date: NaiveDate,
    pub amount: i64,
    pub category_id: Option<Uuid>,
    pub status: String,
    pub event_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionPreview {
    pub as_of_date: NaiveDate,
    pub remaining_before: usize,
    pub records: Vec<Provenance>,
    pub writes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub resolved: usize,
    pub remaining: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyMonth {
    pub month: NaiveDate,
    pub income: i64,
    pub expenses: i64,
    pub net: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyNetBalance {
    pub month: NaiveDate,
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyBalanceReport {
    pub cash_balance: i64,
    pub bank_balance: i64,
    pub total_balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCategoryTotal {
    pub category_id: Uuid,
    pub category_name: String,
    pub total: i64,
}

fn writable(db: &Connection) -> Result<(), AppError> {
    if cutover_date(db)?.is_some() {
        return Err(blocked("Legacy history is read-only after cutover."));
    }
    Ok(())
}

fn transactions(db: &Connection) -> Result<Vec<(Uuid, LegacyTransaction)>, AppError> {
    rows(
        db,
        "SELECT id,amount,description,transaction_date,category_id,account_type
        FROM legacy_transactions ORDER BY transaction_date,id",
        [],
        |r| {
            Ok((
                id(r, 0)?,
                LegacyTransaction {
                    amount: r.get(1)?,
                    description: r.get(2)?,
                    transaction_date: r.get(3)?,
                    category_id: id(r, 4)?,
                    account_type: LegacyAccount::read(r, 5)?,
                },
            ))
        },
    )
}

fn transfers(db: &Connection) -> Result<Vec<(Uuid, LegacyTransfer)>, AppError> {
    rows(
        db,
        "SELECT id,amount,description,transfer_date,from_account_type,to_account_type
        FROM legacy_transfers ORDER BY transfer_date,id",
        [],
        |r| {
            Ok((
                id(r, 0)?,
                LegacyTransfer {
                    amount: r.get(1)?,
                    description: r.get(2)?,
                    transfer_date: r.get(3)?,
                    from: LegacyAccount::read(r, 4)?,
                    to: LegacyAccount::read(r, 5)?,
                },
            ))
        },
    )
}

fn legacy_months(db: &Connection) -> Result<Vec<LegacyMonth>, AppError> {
    let data = rows(
        db,
        "SELECT substr(transaction_date,1,7)||'-01',
        SUM(CASE WHEN c.type='income' THEN l.amount ELSE 0 END),
        SUM(CASE WHEN c.type='expense' THEN l.amount ELSE 0 END)
        FROM legacy_transactions l JOIN categories c ON c.id=l.category_id GROUP BY 1 ORDER BY 1",
        [],
        |r| Ok((r.get(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
    )?;
    data.into_iter()
        .map(|(month, income, expenses)| {
            Ok(LegacyMonth {
                month,
                income: yen(income, "income")?,
                expenses: yen(expenses, "expenses")?,
                net: total([income, -expenses])?,
            })
        })
        .collect()
}

impl Finance<'_> {
    pub fn save_legacy_transaction(
        &self,
        request: Uuid,
        source: Option<Uuid>,
        input: LegacyTransaction,
    ) -> Result<WriteResponse<Uuid>, AppError> {
        self.write(request,"save_legacy_transaction",&(source,&input),|db| {
            writable(db)?;
            date(input.transaction_date)?;
            positive_yen(input.amount,"amount")?;
            let cat = category(db,input.category_id)?;
            if cat.archived_at.is_some() { return Err(invalid("categoryId","Select an active category.")); }
            let description = text(input.description.as_deref(),"description",MAX_DESCRIPTION_CHARS,false)?;
            let record = source.unwrap_or_else(Uuid::new_v4);
            let bindings = params![key(&record),input.amount,description,input.transaction_date,
                key(&input.category_id),input.account_type.as_str()];
            let count = if source.is_some() {
                db.execute("UPDATE legacy_transactions SET amount=?2,description=?3,transaction_date=?4,
                    category_id=?5,account_type=?6 WHERE id=?1",bindings)?
            } else {
                db.execute("INSERT INTO legacy_transactions(id,amount,description,transaction_date,category_id,
                    account_type,recorded_at) VALUES (?1,?2,?3,?4,?5,?6,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",bindings)?
            };
            if count != 1 { return Err(missing()); }
            Ok(record)
        })
    }

    pub fn save_legacy_transfer(
        &self,
        request: Uuid,
        source: Option<Uuid>,
        input: LegacyTransfer,
    ) -> Result<WriteResponse<Uuid>, AppError> {
        self.write(request,"save_legacy_transfer",&(source,&input),|db| {
            writable(db)?;
            date(input.transfer_date)?;
            positive_yen(input.amount,"amount")?;
            if input.from == input.to { return Err(invalid("to","Transfer accounts must be distinct.")); }
            let description = text(input.description.as_deref(),"description",MAX_DESCRIPTION_CHARS,false)?;
            let record = source.unwrap_or_else(Uuid::new_v4);
            let bindings = params![key(&record),input.amount,description,input.transfer_date,input.from.as_str(),input.to.as_str()];
            let count = if source.is_some() {
                db.execute("UPDATE legacy_transfers SET amount=?2,description=?3,transfer_date=?4,
                    from_account_type=?5,to_account_type=?6 WHERE id=?1",bindings)?
            } else {
                db.execute("INSERT INTO legacy_transfers(id,amount,description,transfer_date,from_account_type,to_account_type)
                    VALUES (?1,?2,?3,?4,?5,?6)",bindings)?
            };
            if count != 1 { return Err(missing()); }
            Ok(record)
        })
    }

    pub fn delete_legacy_record(
        &self,
        request: Uuid,
        source: Uuid,
        transfer: bool,
    ) -> Result<WriteResponse<Uuid>, AppError> {
        self.write(request, "delete_legacy_record", &(source, transfer), |db| {
            writable(db)?;
            let sql = if transfer {
                "DELETE FROM legacy_transfers WHERE id=?1"
            } else {
                "DELETE FROM legacy_transactions WHERE id=?1"
            };
            if db.execute(sql, [key(&source)])? == 0 {
                return Err(missing());
            }
            Ok(source)
        })
    }

    pub fn save_legacy_opening(
        &self,
        request: Uuid,
        input: LegacyOpening,
    ) -> Result<WriteResponse<LegacyOpening>, AppError> {
        self.write(request,"save_legacy_opening",&input,|db| {
            writable(db)?;
            date(input.as_of_date)?;
            yen(input.amount,"amount")?;
            db.execute("INSERT INTO legacy_openings(id,account_type,amount,as_of_date) VALUES (?1,?2,?3,?4)
                ON CONFLICT(account_type) DO UPDATE SET amount=excluded.amount,as_of_date=excluded.as_of_date",
                params![key(&Uuid::new_v4()),input.account_type.as_str(),input.amount,input.as_of_date])?;
            Ok(input.clone())
        })
    }

    pub fn legacy_transactions(&self) -> Result<Vec<(Uuid, LegacyTransaction)>, AppError> {
        self.read(transactions)
    }
    pub fn legacy_transfers(&self) -> Result<Vec<(Uuid, LegacyTransfer)>, AppError> {
        self.read(transfers)
    }
    pub fn legacy_monthly(&self) -> Result<Vec<LegacyMonth>, AppError> {
        self.read(legacy_months)
    }

    pub fn legacy_net_balance(&self) -> Result<Vec<LegacyNetBalance>, AppError> {
        self.read(|db| {
            let mut running = amount(
                db,
                "SELECT COALESCE(SUM(amount),0) FROM legacy_openings",
                [],
            )?;
            legacy_months(db)?
                .into_iter()
                .map(|point| {
                    running = total([running, point.net])?;
                    Ok(LegacyNetBalance {
                        month: point.month,
                        balance: running,
                    })
                })
                .collect()
        })
    }

    pub fn legacy_by_category(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<LegacyCategoryTotal>, AppError> {
        report_range(from, to)?;
        self.read(|db| {
            let mut result = rows(
                db,
                "SELECT c.id,c.name,SUM(t.amount) AS expense
                FROM legacy_transactions t JOIN categories c ON c.id=t.category_id
                WHERE c.type='expense' AND (?1 IS NULL OR t.transaction_date>=?1)
                AND (?2 IS NULL OR t.transaction_date<=?2)
                GROUP BY c.id,c.name ORDER BY expense DESC,c.id",
                params![from, to],
                |r| {
                    Ok(LegacyCategoryTotal {
                        category_id: id(r, 0)?,
                        category_name: r.get(1)?,
                        total: r.get(2)?,
                    })
                },
            )?;
            for row in &mut result {
                row.total = yen(row.total, "total")?;
            }
            Ok(result)
        })
    }

    /// Historical legacy report basis, including all recorded legacy movements.
    pub fn legacy_balance_report(&self) -> Result<LegacyBalanceReport, AppError> {
        self.read(|db| {
            let buckets = rows(db, "SELECT account_type,SUM(delta) FROM (
                SELECT account_type,amount AS delta FROM legacy_openings
                UNION ALL SELECT t.account_type,CASE c.type WHEN 'income' THEN t.amount ELSE -t.amount END
                    FROM legacy_transactions t JOIN categories c ON c.id=t.category_id
                UNION ALL SELECT from_account_type,-amount FROM legacy_transfers
                UNION ALL SELECT to_account_type,amount FROM legacy_transfers
            ) GROUP BY account_type",[],|r| Ok((LegacyAccount::read(r,0)?,r.get::<_,i64>(1)?)))?;
            let mut cash_balance=0;
            let mut bank_balance=0;
            for (account,value) in buckets {
                match account {
                    LegacyAccount::Cash => cash_balance=yen(value,"cashBalance")?,
                    LegacyAccount::Bank => bank_balance=yen(value,"bankBalance")?,
                }
            }
            Ok(LegacyBalanceReport { cash_balance,bank_balance,total_balance:total([cash_balance,bank_balance])? })
        })
    }

    pub fn legacy_balance(
        &self,
        bucket: LegacyAccount,
        on: NaiveDate,
    ) -> Result<Option<i64>, AppError> {
        date(on)?;
        self.read(|db| {
            let opening: Option<(i64, NaiveDate)> = db
                .query_row(
                    "SELECT amount,as_of_date FROM legacy_openings WHERE account_type=?1",
                    [bucket.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((opening, start)) = opening.filter(|(_, start)| *start <= on) else {
                return Ok(None);
            };
            let net = amount(
                db,
                "SELECT COALESCE(SUM(CASE WHEN c.type='income' THEN l.amount ELSE -l.amount END),0)
                FROM legacy_transactions l JOIN categories c ON c.id=l.category_id
                WHERE l.account_type=?1 AND l.transaction_date BETWEEN ?2 AND ?3",
                params![bucket.as_str(), start, on],
            )?;
            let moved = amount(
                db,
                "SELECT COALESCE(SUM(CASE WHEN to_account_type=?1 THEN amount ELSE -amount END),0)
                FROM legacy_transfers WHERE (from_account_type=?1 OR to_account_type=?1)
                 AND transfer_date BETWEEN ?2 AND ?3",
                params![bucket.as_str(), start, on],
            )?;
            Ok(Some(total([opening, net, moved])?))
        })
    }

    pub fn preview_cutover(&self, input: &CutoverInput) -> Result<CutoverPreview, AppError> {
        self.read(|db| prepare_cutover(db, input).map(|plan| plan.preview))
    }

    pub fn confirm_cutover(
        &self,
        request: Uuid,
        input: CutoverInput,
    ) -> Result<WriteResponse<Cutover>, AppError> {
        self.write(request,"confirm_cutover",&input,|db| {
            let hash = digest(&input)?;
            if let Some((cutover,prior_hash)) = current_cutover(db)? {
                if hash != prior_hash { return Err(blocked("Cutover was already confirmed with different data.")); }
                return Ok(cutover);
            }
            let CutoverPlan { preview, new_openings, mut mapped } = prepare_cutover(db,&input)?;
            for opening in new_openings {
                add_opening(db,&opening,true)?;
            }
            let cutover = Cutover { id:Uuid::new_v4(),as_of_date:input.as_of_date,
                unresolved_count:preview.unresolved_count,status:"confirmed".into() };
            db.execute("INSERT INTO ledger_cutover VALUES (1,?1,?2,?3,?4,?5)",
                params![key(&cutover.id),input.as_of_date,now(),cutover.unresolved_count,hash])?;
            for mut row in sources(db,input.as_of_date)? {
                if let Some(event) = mapped.remove(&(row.source_table.clone(),row.source_id)) {
                    insert_event(db,&event)?;
                    row.event_id = Some(event.id);
                    row.status = "mapped".into();
                }
                db.execute("INSERT INTO legacy_provenance (id,source_table,source_id,recorded_date,amount,category_id,status,event_id)
                    VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",params![key(&Uuid::new_v4()),row.source_table,key(&row.source_id),
                    row.recorded_date,row.amount,row.category_id.as_ref().map(key),row.status,row.event_id.as_ref().map(key)])?;
            }
            Ok(cutover)
        })
    }

    pub fn provenance(&self, unresolved_only: bool) -> Result<Vec<Provenance>, AppError> {
        self.read(|db| provenance(db, unresolved_only))
    }

    pub fn preview_resolution(
        &self,
        mappings: &LegacyMappings,
    ) -> Result<ResolutionPreview, AppError> {
        self.read(|db| prepare_resolution(db, mappings).map(|p| p.0))
    }

    pub fn confirm_resolution(
        &self,
        request: Uuid,
        mappings: LegacyMappings,
    ) -> Result<WriteResponse<Resolution>, AppError> {
        self.write(request,"resolve_legacy",&mappings,|db| {
            let (preview, prepared) = prepare_resolution(db,&mappings)?;
            if preview.records.iter().all(|r| r.status == "mapped") {
                return Ok(Resolution { resolved:0,remaining:preview.remaining_before,status:"already_mapped".into() });
            }
            if preview.records.iter().any(|r| r.status != "unresolved") {
                return Err(blocked("Submit unresolved and previously mapped records separately."));
            }
            for ((table,source),event) in prepared {
                for posting in &event.postings {
                    super::review::guard_confirmed(db,posting.account_id,posting.effective_date)?;
                }
                insert_event(db,&event)?;
                db.execute("UPDATE legacy_provenance SET status='mapped',event_id=?3 WHERE source_table=?1 AND source_id=?2",
                    params![table,key(&source),key(&event.id)])?;
            }
            let resolved = preview.records.len();
            Ok(Resolution { resolved,remaining:preview.remaining_before-resolved,status:"confirmed".into() })
        })
    }
}

type MappedEvents = BTreeMap<(String, Uuid), Event>;

fn current_cutover(db: &Connection) -> Result<Option<(Cutover, String)>, AppError> {
    Ok(db.query_row("SELECT id,as_of_date,unresolved_count,request_hash FROM ledger_cutover WHERE singleton=1",
        [],|r| Ok((Cutover { id:id(r,0)?,as_of_date:r.get(1)?,unresolved_count:r.get(2)?,
            status:"confirmed".into() },r.get(3)?))).optional()?)
}

fn sources(db: &Connection, cutoff: NaiveDate) -> Result<Vec<Provenance>, AppError> {
    let tx = transactions(db)?.into_iter().map(|(source, r)| {
        (
            "legacy_transactions",
            source,
            r.transaction_date,
            r.amount,
            Some(r.category_id),
        )
    });
    let tr = transfers(db)?
        .into_iter()
        .map(|(source, r)| ("legacy_transfers", source, r.transfer_date, r.amount, None));
    Ok(tx
        .chain(tr)
        .map(|(table, source, recorded, amount, category)| Provenance {
            source_table: table.into(),
            source_id: source,
            recorded_date: recorded,
            amount,
            category_id: category,
            status: if recorded < cutoff {
                "historical"
            } else {
                "unresolved"
            }
            .into(),
            event_id: None,
        })
        .collect())
}

pub(super) fn provenance(db: &Connection, unresolved: bool) -> Result<Vec<Provenance>, AppError> {
    rows(db,"SELECT source_table,source_id,recorded_date,amount,category_id,status,event_id FROM legacy_provenance
        WHERE NOT ?1 OR status='unresolved' ORDER BY recorded_date,source_table,source_id",[unresolved],|r| Ok(Provenance {
        source_table:r.get(0)?,source_id:id(r,1)?,recorded_date:r.get(2)?,amount:r.get(3)?,category_id:optional_id(r,4)?,
        status:r.get(5)?,event_id:optional_id(r,6)?,
    }))
}

struct CutoverPlan {
    preview: CutoverPreview,
    new_openings: Vec<OpeningInput>,
    mapped: MappedEvents,
}

fn prepare_cutover(db: &Connection, input: &CutoverInput) -> Result<CutoverPlan, AppError> {
    let boundary = input.as_of_date;
    date(boundary)?;
    let active_accounts = accounts(db, false)?;
    let opening_ids: BTreeSet<_> = input.openings.iter().map(|o| o.account_id).collect();
    if active_accounts.is_empty()
        || opening_ids.len() != input.openings.len()
        || opening_ids != active_accounts.iter().map(|a| a.id).collect()
    {
        return Err(invalid(
            "openings",
            "Every active named account requires one explicit opening.",
        ));
    }
    if exists(
        db,
        "SELECT 1 FROM account_postings WHERE effective_date<?1
         UNION ALL SELECT 1 FROM financial_events WHERE occurred_on<?1",
        [boundary],
    )? {
        return Err(blocked(
            "The cutover date would exclude existing canonical activity.",
        ));
    }
    let posted_accounts: BTreeSet<_> = rows(
        db,
        "SELECT DISTINCT account_id FROM account_postings",
        [],
        |r| id(r, 0),
    )?
    .into_iter()
    .collect();
    let mut new_openings = Vec::new();
    let mut opening_dates = BTreeMap::new();
    for opening in &input.openings {
        yen(opening.amount, "amount")?;
        let start = if posted_accounts.contains(&opening.account_id) {
            let existing = latest_opening(db, opening.account_id)?
                .filter(|o| o.amount == opening.amount && o.as_of_date >= boundary)
                .ok_or_else(|| blocked("Keep the existing opening amount and choose a cutover date on or before that opening."))?;
            existing.as_of_date
        } else {
            new_openings.push(OpeningInput {
                account_id: opening.account_id,
                amount: opening.amount,
                as_of_date: boundary,
                reason: "Legacy cutover".into(),
            });
            boundary
        };
        opening_dates.insert(opening.account_id, start);
    }
    let mapped = prepare_mappings(db, boundary, &input.mappings, |id| {
        Ok(opening_dates.get(&id).copied())
    })?;
    for posting in mapped.values().flat_map(|event| &event.postings) {
        super::review::guard_confirmed(db, posting.account_id, posting.effective_date)?;
    }
    let mut preview = CutoverPreview {
        as_of_date: input.as_of_date,
        legacy_transaction_count: 0,
        legacy_transfer_count: 0,
        category_sums: BTreeMap::new(),
        legacy_date_min: None,
        legacy_date_max: None,
        unresolved_transaction_ids: Vec::new(),
        unresolved_transfer_ids: Vec::new(),
        unresolved_count: 0,
        mapped_count: mapped.len(),
        opening_total: total(input.openings.iter().map(|r| r.amount))?,
        legacy_account_type_is_unverified: true,
        purchase_date_basis: "unknown_unless_confirmed".into(),
    };
    for row in sources(db, input.as_of_date)? {
        preview.legacy_date_min = Some(
            preview
                .legacy_date_min
                .map_or(row.recorded_date, |d| d.min(row.recorded_date)),
        );
        preview.legacy_date_max = Some(
            preview
                .legacy_date_max
                .map_or(row.recorded_date, |d| d.max(row.recorded_date)),
        );
        let list = if let Some(cat) = row.category_id {
            preview.legacy_transaction_count += 1;
            let value = preview.category_sums.entry(cat).or_default();
            *value = total([*value, row.amount])?;
            &mut preview.unresolved_transaction_ids
        } else {
            preview.legacy_transfer_count += 1;
            &mut preview.unresolved_transfer_ids
        };
        if row.status == "unresolved" && !mapped.contains_key(&(row.source_table, row.source_id)) {
            list.push(row.source_id);
            preview.unresolved_count += 1;
        }
    }
    Ok(CutoverPlan {
        preview,
        new_openings,
        mapped,
    })
}

fn prepare_resolution(
    db: &Connection,
    mappings: &LegacyMappings,
) -> Result<(ResolutionPreview, MappedEvents), AppError> {
    let (cutover, _) = current_cutover(db)?.ok_or_else(missing)?;
    let prepared = prepare_mappings(db, cutover.as_of_date, mappings, |id| {
        Ok(latest_opening(db, id)?.map(|o| o.as_of_date))
    })?;
    if prepared.is_empty() {
        return Err(invalid("mappings", "Select an unresolved record."));
    }
    let all = provenance(db, false)?;
    let remaining_before = all.iter().filter(|r| r.status == "unresolved").count();
    let mut records = Vec::new();
    for r in all {
        let Some(candidate) = prepared.get(&(r.source_table.clone(), r.source_id)) else {
            continue;
        };
        if r.status == "historical" {
            return Err(missing());
        }
        if let Some(event_id) = r.event_id {
            let saved = event(db, event_id)?;
            if saved.postings != candidate.postings || saved.occurred_on != candidate.occurred_on {
                return Err(conflict(
                    codes::UNRESOLVED_MAPPING,
                    "The record was mapped with different evidence.",
                ));
            }
        }
        records.push(r);
    }
    if records.len() != prepared.len() {
        return Err(missing());
    }
    Ok((
        ResolutionPreview {
            as_of_date: cutover.as_of_date,
            remaining_before,
            records,
            writes: 0,
        },
        prepared,
    ))
}

fn prepare_mappings(
    db: &Connection,
    cutoff: NaiveDate,
    mappings: &LegacyMappings,
    opening_date: impl Fn(Uuid) -> Result<Option<NaiveDate>, AppError>,
) -> Result<MappedEvents, AppError> {
    let tx: BTreeMap<_, _> = transactions(db)?.into_iter().collect();
    let tr: BTreeMap<_, _> = transfers(db)?.into_iter().collect();
    let mut result = BTreeMap::new();
    for m in &mappings.transactions {
        let row = tx.get(&m.transaction_id).ok_or_else(missing)?;
        let a = account(db, m.account_id)?;
        active(&a)?;
        validate_mapping_date(
            cutoff,
            row.transaction_date,
            m.effective_date,
            opening_date(a.id)?,
        )?;
        if let Some(occurred) = m.occurred_on {
            date(occurred)?;
            if occurred < cutoff {
                return Err(invalid(
                    "occurredOn",
                    "Pre-cutover purchases cannot be posted.",
                ));
            }
        }
        let kind = category(db, row.category_id)?.kind;
        if a.kind == AccountKind::BrokerageCash {
            return Err(invalid(
                "accountId",
                "Map legacy transactions to cash, bank or card.",
            ));
        }
        if a.kind == AccountKind::CreditCard
            && (kind == CategoryType::Income || m.occurred_on != Some(m.effective_date))
        {
            return Err(invalid(
                "occurredOn",
                "Card mapping needs a confirmed purchase date equal to the effective date.",
            ));
        }
        let amount = positive_yen(row.amount, "amount")?;
        let (kind, delta) = if kind == CategoryType::Income {
            (EventKind::Income, amount)
        } else {
            (EventKind::Expense, -amount)
        };
        let mapped = mapped_event(
            kind,
            amount,
            Some(row.category_id),
            row.description.clone(),
            m.occurred_on,
            vec![Posting {
                account_id: a.id,
                delta,
                effective_date: m.effective_date,
            }],
        );
        if result
            .insert(("legacy_transactions".into(), m.transaction_id), mapped)
            .is_some()
        {
            return Err(invalid("transactions", "A record can be mapped only once."));
        }
    }
    for m in &mappings.transfers {
        let row = tr.get(&m.transfer_id).ok_or_else(missing)?;
        let source = account(db, m.from_account_id)?;
        let dest = account(db, m.to_account_id)?;
        for a in [&source, &dest] {
            active(a)?;
            if !matches!(a.kind, AccountKind::Cash | AccountKind::Bank) {
                return Err(invalid(
                    "accountId",
                    "Map transfers between cash and bank accounts.",
                ));
            }
            validate_mapping_date(
                cutoff,
                row.transfer_date,
                m.effective_date,
                opening_date(a.id)?,
            )?;
        }
        if source.id == dest.id {
            return Err(invalid(
                "toAccountId",
                "Transfer accounts must be distinct.",
            ));
        }
        let amount = positive_yen(row.amount, "amount")?;
        let mapped = mapped_event(
            EventKind::Transfer,
            amount,
            None,
            row.description.clone(),
            None,
            vec![
                Posting {
                    account_id: source.id,
                    delta: -amount,
                    effective_date: m.effective_date,
                },
                Posting {
                    account_id: dest.id,
                    delta: amount,
                    effective_date: m.effective_date,
                },
            ],
        );
        if result
            .insert(("legacy_transfers".into(), m.transfer_id), mapped)
            .is_some()
        {
            return Err(invalid("transfers", "A record can be mapped only once."));
        }
    }
    Ok(result)
}

fn validate_mapping_date(
    cutoff: NaiveDate,
    recorded: NaiveDate,
    effective: NaiveDate,
    opening: Option<NaiveDate>,
) -> Result<(), AppError> {
    date(effective)?;
    if recorded < cutoff || effective < cutoff {
        return Err(invalid(
            "effectiveDate",
            "Pre-cutover records are already represented by the opening.",
        ));
    }
    if opening.is_none_or(|on| effective < on) {
        return Err(AppError::invalid(
            codes::MISSING_OPENING,
            "effectiveDate",
            "Mapped account needs an opening.",
        ));
    }
    Ok(())
}

fn mapped_event(
    kind: EventKind,
    amount: i64,
    category_id: Option<Uuid>,
    description: Option<String>,
    occurred_on: Option<NaiveDate>,
    mut postings: Vec<Posting>,
) -> Event {
    postings.sort_by_key(|p| p.account_id);
    Event {
        id: Uuid::new_v4(),
        kind,
        amount,
        category_id,
        description,
        occurred_on,
        date_basis: if occurred_on.is_some() {
            DateBasis::ConfirmedPurchaseDate
        } else {
            DateBasis::LegacyRecordedDate
        },
        recorded_at: now(),
        original_event_id: None,
        reverses_event_id: None,
        replaces_event_id: None,
        is_reversal: false,
        postings,
    }
}
