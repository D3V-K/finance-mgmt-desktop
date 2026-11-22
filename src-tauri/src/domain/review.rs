use super::ledger::*;
use super::*;
use crate::commands::wire::MAX_DESCRIPTION_CHARS;
use chrono::{Datelike, Months};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValuationInput {
    pub account_id: Uuid,
    pub as_of_date: NaiveDate,
    pub market_value: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Valuation {
    pub id: Uuid,
    #[serde(flatten)]
    pub input: ValuationInput,
    pub scope: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsChange {
    pub account_id: Uuid,
    pub occurred_on: NaiveDate,
    pub reason: String,
    pub user_entered_realized_gain: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecuritiesPosition {
    pub value: Option<i64>,
    pub as_of_date: Option<NaiveDate>,
    pub scope: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PositionAccount {
    pub account_id: Uuid,
    pub name: String,
    pub kind: AccountKind,
    pub balance: Option<i64>,
    pub status: BalanceStatus,
    pub securities: Option<SecuritiesPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub as_of_date: NaiveDate,
    pub currency: String,
    pub liquid_cash: Option<i64>,
    pub brokerage_cash: Option<i64>,
    pub securities_value: Option<i64>,
    pub signed_card_balances: Option<i64>,
    pub net_worth: Option<i64>,
    pub coverage: String,
    pub accounts: Vec<PositionAccount>,
    pub cumulative_contributions: i64,
    pub cumulative_withdrawals: i64,
    pub user_entered_realized_gain: i64,
    pub valuation_change: Option<i64>,
    pub investment_return: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationInput {
    pub account_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub statement_opening: i64,
    pub statement_closing: i64,
    pub selected_posting_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPosting {
    pub id: Uuid,
    pub event_id: Uuid,
    pub delta: i64,
    pub effective_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    pub id: Uuid,
    pub account_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub statement_opening: i64,
    pub statement_closing: i64,
    pub calculated_closing: i64,
    pub all_postings_closing: i64,
    pub difference: i64,
    pub baseline_before_period: Option<i64>,
    pub opening_mismatch: bool,
    pub selected_posting_ids: BTreeSet<Uuid>,
    pub unmatched_postings: Vec<ReviewPosting>,
    pub status: String,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationAudit {
    pub action: String,
    pub posting_id: Option<Uuid>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualityIssue {
    pub kind: String,
    pub record_id: Uuid,
}

fn brokerage(db: &Connection, id: Uuid) -> Result<Account, AppError> {
    let a = account(db, id)?;
    if a.kind != AccountKind::BrokerageCash {
        return Err(invalid("accountId", "Select a brokerage cash account."));
    }
    Ok(a)
}

fn valuations(
    db: &Connection,
    account_id: Uuid,
    through: Option<NaiveDate>,
    limit: i64,
) -> Result<Vec<Valuation>, AppError> {
    rows(db,"SELECT id,as_of_date,market_value,note,recorded_at FROM securities_valuations WHERE account_id=?1
        AND (?2 IS NULL OR as_of_date<=?2) ORDER BY as_of_date DESC LIMIT ?3",
        params![key(&account_id),through,limit],|r|Ok(Valuation {
        id:id(r,0)?,input:ValuationInput{account_id,as_of_date:r.get(1)?,market_value:r.get(2)?,note:r.get(3)?},
        scope:"securities_only".into(),recorded_at:r.get(4)?,
    }))
}

fn securities(
    db: &Connection,
    account_id: Uuid,
    on: NaiveDate,
) -> Result<(SecuritiesPosition, Option<i64>), AppError> {
    let snapshots = valuations(db, account_id, Some(on), 2)?;
    let Some(current) = snapshots.first() else {
        return Ok((
            SecuritiesPosition {
                value: None,
                as_of_date: None,
                scope: "securities_only".into(),
                status: "missing".into(),
            },
            None,
        ));
    };
    let changed = exists(
        db,
        "SELECT 1 FROM holdings_changes WHERE account_id=?1 AND occurred_on<=?2
        AND (occurred_on>?3 OR (occurred_on=?3 AND recorded_at>?4))",
        params![
            key(&account_id),
            on,
            current.input.as_of_date,
            current.recorded_at
        ],
    )?;
    let change = snapshots
        .get(1)
        .map(|p| current.input.market_value - p.input.market_value);
    Ok((
        SecuritiesPosition {
            value: Some(current.input.market_value),
            as_of_date: Some(current.input.as_of_date),
            scope: current.scope.clone(),
            status: if changed {
                "stale_after_holdings_change"
            } else {
                "recorded"
            }
            .into(),
        },
        change,
    ))
}

pub(super) fn position(
    db: &Connection,
    on: NaiveDate,
    mapping_complete: bool,
) -> Result<Position, AppError> {
    date(on)?;
    let all = accounts(db, true)?;
    let mut result = Position {
        as_of_date: on,
        currency: "JPY".into(),
        liquid_cash: None,
        brokerage_cash: None,
        securities_value: None,
        signed_card_balances: None,
        net_worth: None,
        coverage: "incomplete".into(),
        accounts: Vec::new(),
        cumulative_contributions: 0,
        cumulative_withdrawals: 0,
        user_entered_realized_gain: 0,
        valuation_change: None,
        investment_return: None,
    };
    let mut complete = !all.is_empty();
    let (mut liquid, mut cash, mut debt, mut investments, mut changes) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut changes_known = true;
    for a in all {
        let b = balance(db, &a, on, mapping_complete)?;
        complete &= b.status == BalanceStatus::Complete;
        match (b.balance, a.kind) {
            (Some(v), AccountKind::CreditCard) => debt.push(v),
            (Some(v), AccountKind::BrokerageCash) => cash.push(v),
            (Some(v), _) => liquid.push(v),
            _ => complete = false,
        }
        let securities = if a.kind == AccountKind::BrokerageCash {
            let (snapshot, change) = securities(db, a.id, on)?;
            complete &= snapshot.status == "recorded";
            changes_known &= change.is_some();
            changes.extend(change);
            investments.extend(snapshot.value);
            Some(snapshot)
        } else {
            None
        };
        result.accounts.push(PositionAccount {
            account_id: a.id,
            name: a.name,
            kind: a.kind,
            balance: b.balance,
            status: b.status,
            securities,
        });
    }
    let movements = rows(
        db,
        "SELECT p.delta,e.is_reversal FROM account_postings p
        JOIN financial_events e ON e.id=p.event_id JOIN accounts a ON a.id=p.account_id
        WHERE a.kind='BROKERAGE_CASH' AND e.kind='TRANSFER' AND p.effective_date<=?1",
        [on],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)),
    )?;
    result.cumulative_contributions = total(
        movements
            .iter()
            .filter(|(v, r)| (*v > 0) != *r)
            .map(|(v, _)| *v),
    )?;
    result.cumulative_withdrawals = total(
        movements
            .iter()
            .filter(|(v, r)| (*v < 0) != *r)
            .map(|(v, _)| -v),
    )?;
    result.user_entered_realized_gain = amount(
        db,
        "SELECT COALESCE(SUM(realized_gain),0) FROM holdings_changes WHERE occurred_on<=?1",
        [on],
    )?;
    if complete {
        let (liquid, cash, debt, investments) = (
            total(liquid)?,
            total(cash)?,
            total(debt)?,
            total(investments)?,
        );
        result.liquid_cash = Some(liquid);
        result.brokerage_cash = Some(cash);
        result.signed_card_balances = Some(debt);
        result.securities_value = Some(investments);
        result.net_worth = Some(total([liquid, cash, debt, investments])?);
        result.coverage = "complete".into();
        if changes_known {
            result.valuation_change = Some(total(changes)?);
        }
    }
    Ok(result)
}

fn posting_rows(
    db: &Connection,
    id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ReviewPosting>, AppError> {
    rows(
        db,
        "SELECT id,event_id,delta,effective_date FROM account_postings
        WHERE account_id=?1 AND effective_date BETWEEN ?2 AND ?3 ORDER BY effective_date,id",
        params![key(&id), start, end],
        |r| {
            Ok(ReviewPosting {
                id: super::ledger::id(r, 0)?,
                event_id: super::ledger::id(r, 1)?,
                delta: r.get(2)?,
                effective_date: r.get(3)?,
            })
        },
    )
}

fn review_values(
    db: &Connection,
    session: Uuid,
    input: &ReconciliationInput,
    selection: BTreeSet<Uuid>,
    status: String,
    captured_at: String,
) -> Result<Reconciliation, AppError> {
    let postings = posting_rows(db, input.account_id, input.period_start, input.period_end)?;
    let all =
        total(std::iter::once(input.statement_opening).chain(postings.iter().map(|p| p.delta)))?;
    let calculated = total(
        std::iter::once(input.statement_opening).chain(
            postings
                .iter()
                .filter(|p| selection.contains(&p.id))
                .map(|p| p.delta),
        ),
    )?;
    let difference = total([input.statement_closing, -calculated])?;
    let unmatched_postings: Vec<_> = postings
        .into_iter()
        .filter(|p| !selection.contains(&p.id))
        .collect();
    let a = account(db, input.account_id)?;
    let prior = balance(
        db,
        &a,
        input
            .period_start
            .pred_opt()
            .ok_or_else(|| invalid("periodStart", "Statement start is too early."))?,
        mapping_complete(db)?,
    )?;
    let opening_mismatch = prior.balance.is_some_and(|b| b != input.statement_opening);
    let stale = status == "confirmed"
        && (difference != 0 || !unmatched_postings.is_empty() || opening_mismatch);
    Ok(Reconciliation {
        id: session,
        account_id: input.account_id,
        period_start: input.period_start,
        period_end: input.period_end,
        statement_opening: input.statement_opening,
        statement_closing: input.statement_closing,
        calculated_closing: calculated,
        all_postings_closing: all,
        difference,
        baseline_before_period: prior.balance,
        opening_mismatch,
        selected_posting_ids: selection,
        unmatched_postings,
        status: if stale { "stale".into() } else { status },
        captured_at,
    })
}

fn selected_for(db: &Connection, input: &ReconciliationInput) -> Result<BTreeSet<Uuid>, AppError> {
    range(input.period_start, input.period_end, 370)?;
    yen(input.statement_opening, "statementOpening")?;
    yen(input.statement_closing, "statementClosing")?;
    account(db, input.account_id)?;
    let all: BTreeSet<_> =
        posting_rows(db, input.account_id, input.period_start, input.period_end)?
            .into_iter()
            .map(|p| p.id)
            .collect();
    if let Some(chosen) = &input.selected_posting_ids {
        let selected: BTreeSet<_> = chosen.iter().copied().collect();
        if selected.len() != chosen.len() || !selected.is_subset(&all) {
            return Err(invalid(
                "selectedPostingIds",
                "Select distinct postings from this account and period.",
            ));
        }
        Ok(selected)
    } else {
        Ok(all)
    }
}

fn reconciliation(db: &Connection, session: Uuid) -> Result<Reconciliation, AppError> {
    let (input,status,stamp)=db.query_row("SELECT account_id,period_start,period_end,statement_opening,statement_closing,status,captured_at
        FROM reconciliations WHERE id=?1",[key(&session)],|r|Ok((ReconciliationInput { account_id:id(r,0)?,period_start:r.get(1)?,
        period_end:r.get(2)?,statement_opening:r.get(3)?,statement_closing:r.get(4)?,selected_posting_ids:None },
        r.get(5)?,r.get(6)?)))?;
    let selected = rows(
        db,
        "SELECT posting_id FROM reconciled_postings WHERE session_id=?1",
        [key(&session)],
        |r| id(r, 0),
    )?
    .into_iter()
    .collect();
    review_values(db, session, &input, selected, status, stamp)
}

fn log_review(
    db: &Connection,
    session: Uuid,
    action: &str,
    posting: Option<Uuid>,
) -> Result<(), AppError> {
    db.execute("INSERT INTO reconciliation_audit (session_id,action,posting_id,recorded_at) VALUES (?1,?2,?3,?4)",
        params![key(&session),action,posting.as_ref().map(key),now()])?;
    Ok(())
}

pub(super) fn guard_confirmed(
    db: &Connection,
    account_id: Uuid,
    on: NaiveDate,
) -> Result<(), AppError> {
    if exists(
        db,
        "SELECT 1 FROM reconciliations WHERE account_id=?1 AND status='confirmed' AND period_end>=?2",
        params![key(&account_id), on],
    )? {
        return Err(blocked(
            "Reopen affected reconciliations before changing confirmed balances.",
        ));
    }
    Ok(())
}

impl Finance<'_> {
    pub fn record_valuation(
        &self,
        request: Uuid,
        input: ValuationInput,
    ) -> Result<WriteResponse<Valuation>, AppError> {
        self.write(request, "record_valuation", &input, |db| {
            active(&brokerage(db, input.account_id)?)?;
            date(input.as_of_date)?;
            nonnegative(input.market_value, "marketValue")?;
            let note = text(input.note.as_deref(), "note", MAX_DESCRIPTION_CHARS, false)?;
            if exists(
                db,
                "SELECT 1 FROM securities_valuations WHERE account_id=?1 AND as_of_date=?2",
                params![key(&input.account_id), input.as_of_date],
            )? {
                return Err(blocked(
                    "A securities valuation already exists on this date.",
                ));
            }
            let id = Uuid::new_v4();
            let recorded_at = now();
            db.execute(
                "INSERT INTO securities_valuations VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    key(&id),
                    key(&input.account_id),
                    input.as_of_date,
                    input.market_value,
                    note,
                    recorded_at
                ],
            )?;
            Ok(Valuation {
                id,
                input: ValuationInput {
                    note,
                    ..input.clone()
                },
                scope: "securities_only".into(),
                recorded_at,
            })
        })
    }

    pub fn valuations(&self, account: Uuid) -> Result<Vec<Valuation>, AppError> {
        self.read(|db| {
            brokerage(db, account)?;
            valuations(db, account, None, -1)
        })
    }

    pub fn mark_holdings_change(
        &self,
        request: Uuid,
        input: HoldingsChange,
    ) -> Result<WriteResponse<Uuid>, AppError> {
        self.write(request, "mark_holdings_change", &input, |db| {
            active(&brokerage(db, input.account_id)?)?;
            date(input.occurred_on)?;
            let reason = label(&input.reason, "reason", MAX_DESCRIPTION_CHARS)?;
            if let Some(value) = input.user_entered_realized_gain {
                yen(value, "userEnteredRealizedGain")?;
            }
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO holdings_changes VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    key(&id),
                    key(&input.account_id),
                    input.occurred_on,
                    reason,
                    input.user_entered_realized_gain,
                    now()
                ],
            )?;
            Ok(id)
        })
    }

    pub fn financial_position(&self, on: NaiveDate) -> Result<Position, AppError> {
        self.read(|db| position(db, on, mapping_complete(db)?))
    }

    pub fn position_history(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Position>, AppError> {
        range(from, to, 366 * 10)?;
        self.read(|db| {
            let mapping_complete = mapping_complete(db)?;
            let mut results = Vec::new();
            let mut month = from.with_day(1).expect("month has first day");
            while month <= to {
                let next = month
                    .checked_add_months(Months::new(1))
                    .ok_or_else(|| invalid("to", "Date range is too large."))?;
                let on = next.pred_opt().expect("month has previous day").min(to);
                results.push(position(db, on, mapping_complete)?);
                month = next;
            }
            Ok(results)
        })
    }

    pub fn preview_reconciliation(
        &self,
        input: &ReconciliationInput,
    ) -> Result<Reconciliation, AppError> {
        self.read(|db| {
            review_values(
                db,
                Uuid::nil(),
                input,
                selected_for(db, input)?,
                "preview".into(),
                String::new(),
            )
        })
    }

    pub fn create_reconciliation(
        &self,
        request: Uuid,
        input: ReconciliationInput,
    ) -> Result<WriteResponse<Reconciliation>, AppError> {
        self.write(request,"create_reconciliation",&input,|db| {
            let selected=selected_for(db,&input)?;
            if exists(db,"SELECT 1 FROM reconciliations WHERE account_id=?1 AND period_start=?2 AND period_end=?3",
                params![key(&input.account_id),input.period_start,input.period_end])? {
                return Err(blocked("This account already has a reconciliation for that period."));
            }
            let session=Uuid::new_v4();
            db.execute("INSERT INTO reconciliations VALUES (?1,?2,?3,?4,?5,?6,'draft',?7)",
                params![key(&session),key(&input.account_id),input.period_start,input.period_end,input.statement_opening,
                    input.statement_closing,now()])?;
            for posting in selected {
                db.execute("INSERT INTO reconciled_postings VALUES (?1,?2)",params![key(&session),key(&posting)])?;
            }
            log_review(db,session,"draft_created",None)?;
            reconciliation(db,session)
        })
    }

    pub fn reconciliation(&self, session: Uuid) -> Result<Reconciliation, AppError> {
        self.read(|db| reconciliation(db, session))
    }

    pub fn set_reconciled_posting(
        &self,
        request: Uuid,
        session: Uuid,
        posting: Uuid,
        included: bool,
    ) -> Result<WriteResponse<Reconciliation>, AppError> {
        self.write(request,"set_reconciled_posting",&(session,posting,included),|db| {
            let prior=reconciliation(db,session)?;
            if prior.status!="draft" { return Err(blocked("Reopen the statement before changing its postings.")); }
            if !exists(db,"SELECT 1 FROM account_postings WHERE id=?1 AND account_id=?2 AND effective_date BETWEEN ?3 AND ?4",
                params![key(&posting),key(&prior.account_id),prior.period_start,prior.period_end])? {
                return Err(missing());
            }
            let (sql,action)=if included {
                ("INSERT INTO reconciled_postings VALUES (?1,?2) ON CONFLICT DO NOTHING","posting_included")
            } else {
                ("DELETE FROM reconciled_postings WHERE session_id=?1 AND posting_id=?2","posting_excluded")
            };
            if db.execute(sql,params![key(&session),key(&posting)])?>0 { log_review(db,session,action,Some(posting))?; }
            reconciliation(db,session)
        })
    }

    pub fn confirm_reconciliation(
        &self,
        request: Uuid,
        session: Uuid,
    ) -> Result<WriteResponse<Reconciliation>, AppError> {
        self.write(request,"confirm_reconciliation",&session,|db| {
            let prior=reconciliation(db,session)?;
            if prior.status=="confirmed" { return Ok(prior); }
            if prior.status=="stale" { return Err(blocked("Reopen the stale statement to review its changes.")); }
            if prior.difference!=0 || prior.opening_mismatch || !prior.unmatched_postings.is_empty() {
                return Err(blocked("Resolve the difference, opening mismatch and unmatched postings before confirmation."));
            }
            db.execute("UPDATE reconciliations SET status='confirmed' WHERE id=?1",[key(&session)])?;
            log_review(db,session,"confirmed",None)?;
            reconciliation(db,session)
        })
    }

    pub fn reopen_reconciliation(
        &self,
        request: Uuid,
        session: Uuid,
    ) -> Result<WriteResponse<Reconciliation>, AppError> {
        self.write(request, "reopen_reconciliation", &session, |db| {
            reconciliation(db, session)?;
            if db.execute(
                "UPDATE reconciliations SET status='draft' WHERE id=?1 AND status='confirmed'",
                [key(&session)],
            )? > 0
            {
                log_review(db, session, "reopened", None)?;
            }
            reconciliation(db, session)
        })
    }

    pub fn reconciliation_audit(
        &self,
        session: Uuid,
    ) -> Result<Vec<ReconciliationAudit>, AppError> {
        self.read(|db| {
            reconciliation(db,session)?;
            rows(db,"SELECT action,posting_id,recorded_at FROM reconciliation_audit WHERE session_id=?1 ORDER BY id",[key(&session)],
                |r|Ok(ReconciliationAudit { action:r.get(0)?,posting_id:optional_id(r,1)?,recorded_at:r.get(2)? }))
        })
    }

    pub fn ledger_quality(&self, on: NaiveDate) -> Result<Vec<QualityIssue>, AppError> {
        date(on)?;
        self.read(|db| {
            let mut issues=rows(db,"SELECT kind,id FROM (
                SELECT 'unknown_purchase_date' AS kind,id FROM financial_events e WHERE kind='EXPENSE'
                AND occurred_on IS NULL AND NOT is_reversal AND NOT EXISTS(SELECT 1 FROM financial_events r WHERE r.reverses_event_id=e.id)
                UNION ALL SELECT 'unresolved_mapping',source_id FROM legacy_provenance WHERE status='unresolved'
                UNION ALL SELECT 'missing_opening',id FROM accounts a WHERE NOT EXISTS(
                    SELECT 1 FROM account_opening_revisions o WHERE account_id=a.id)
                UNION ALL SELECT 'missing_settlement_account',s.id FROM card_schedules s
                    WHERE NOT EXISTS(SELECT 1 FROM card_settings c WHERE c.card_id=s.card_id)
                ) ORDER BY kind,id",[],|r|Ok(QualityIssue { kind:r.get(0)?,record_id:id(r,1)? }))?;
            for a in position(db,on,mapping_complete(db)?)?.accounts {
                if let Some(s)=a.securities.filter(|s|s.status!="recorded") {
                    issues.push(QualityIssue { kind:format!("valuation_{}",s.status),record_id:a.account_id });
                }
            }
            for session in rows(db,"SELECT id FROM reconciliations ORDER BY id",[],|r|id(r,0))? {
                let status=reconciliation(db,session)?.status;
                if status!="confirmed" {
                    issues.push(QualityIssue { kind:format!("reconciliation_{status}"),record_id:session });
                }
            }
            issues.extend(super::subscriptions::quality_issues(db, on)?);
            Ok(issues)
        })
    }
}

pub(super) fn nonnegative(value: i64, field: &str) -> Result<i64, AppError> {
    yen(value, field)?;
    if value < 0 {
        return Err(AppError::invalid(
            codes::INVALID_AMOUNT,
            field,
            "Enter a nonnegative amount.",
        ));
    }
    Ok(value)
}
