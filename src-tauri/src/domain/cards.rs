use super::ledger::*;
use super::*;
use crate::commands::wire::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CardSettings {
    pub card_id: Uuid,
    pub settlement_account_id: Uuid,
    pub closing_day: u32,
    pub payment_day: u32,
}

stored_enum!(ScheduleSource { Estimate => "estimate", Actual => "actual" });

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInput {
    pub card_id: Uuid,
    pub purchase_event_id: Option<Uuid>,
    pub opening_obligation: Option<String>,
    pub due_date: NaiveDate,
    pub principal_due: i64,
    pub source: ScheduleSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: Uuid,
    #[serde(flatten)]
    pub terms: ScheduleInput,
    pub statement_id: Option<Uuid>,
    pub paid: i64,
    pub refunded: i64,
    pub outstanding: i64,
    pub status: String,
    pub funded_reserve: i64,
    pub unfunded: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatementInput {
    pub card_id: Uuid,
    pub closing_on: NaiveDate,
    pub due_date: NaiveDate,
    pub principal_confirmed: i64,
    pub schedule_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Statement {
    pub id: Uuid,
    #[serde(flatten)]
    pub terms: StatementInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavingsGoalInput {
    pub name: String,
    pub target: i64,
    pub target_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavingsGoal {
    pub id: Uuid,
    #[serde(flatten)]
    pub terms: SavingsGoalInput,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavingsBucket {
    pub id: Uuid,
    pub name: String,
    pub goal_id: Option<Uuid>,
    pub funded: i64,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Funding {
    pub account_id: Uuid,
    pub amount: i64,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AllocationCapacity {
    pub balance: Balance,
    pub funded: i64,
    pub allocatable: Option<i64>,
    pub shortfall: Option<i64>,
}

pub(super) fn day_of_month(day: u32) -> Result<(), AppError> {
    if !(1..=31).contains(&day) {
        return Err(invalid("day", "Enter a day from 1 through 31."));
    }
    Ok(())
}

fn card(db: &Connection, card_id: Uuid) -> Result<Account, AppError> {
    let account = account(db, card_id)?;
    if account.kind != AccountKind::CreditCard {
        return Err(invalid("cardId", "Select a credit card."));
    }
    Ok(account)
}

pub(super) fn funded(
    db: &Connection,
    account_id: Uuid,
    through: Option<NaiveDate>,
) -> Result<i64, AppError> {
    amount(
        db,
        "SELECT COALESCE(SUM(amount),0) FROM funding_allocations WHERE account_id=?1
        AND (?2 IS NULL OR effective_date<=?2)",
        params![key(&account_id), through],
    )
}

pub(super) fn schedule_funding(
    db: &Connection,
    schedule_id: Uuid,
    account: Option<Uuid>,
    through: Option<NaiveDate>,
) -> Result<i64, AppError> {
    amount(
        db,
        "SELECT COALESCE(SUM(amount),0) FROM funding_allocations WHERE schedule_id=?1
        AND (?2 IS NULL OR account_id=?2) AND (?3 IS NULL OR effective_date<=?3)",
        params![key(&schedule_id), account.as_ref().map(key), through],
    )
}

pub(super) fn schedule(
    db: &Connection,
    schedule_id: Uuid,
    through: Option<NaiveDate>,
) -> Result<Schedule, AppError> {
    let (terms, statement_id) = db.query_row(
        "SELECT card_id,purchase_event_id,opening_obligation,due_date,principal_due,
        source,statement_id FROM card_schedules WHERE id=?1",
        [key(&schedule_id)],
        |r| {
            Ok((
                ScheduleInput {
                    card_id: id(r, 0)?,
                    purchase_event_id: optional_id(r, 1)?,
                    opening_obligation: r.get(2)?,
                    due_date: r.get(3)?,
                    principal_due: r.get(4)?,
                    source: ScheduleSource::read(r, 5)?,
                },
                optional_id(r, 6)?,
            ))
        },
    )?;
    let (paid, refunded): (i64, i64) = db.query_row(
        "SELECT
        COALESCE(SUM(CASE WHEN kind='payment' THEN amount ELSE 0 END),0),
        COALESCE(SUM(CASE WHEN kind='refund' THEN amount ELSE 0 END),0)
        FROM card_applications WHERE schedule_id=?1 AND (?2 IS NULL OR effective_date<=?2)",
        params![key(&schedule_id), through],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let outstanding = total([terms.principal_due, -paid, -refunded])?;
    let funded_reserve = schedule_funding(db, schedule_id, None, through)?;
    let status = if outstanding == 0 {
        "paid"
    } else if paid > 0 {
        "partially_paid"
    } else if terms.source == ScheduleSource::Actual {
        "confirmed"
    } else {
        "planned"
    };
    Ok(Schedule {
        id: schedule_id,
        terms,
        statement_id,
        paid,
        refunded,
        outstanding,
        status: status.into(),
        funded_reserve,
        unfunded: (outstanding - funded_reserve).max(0),
    })
}

pub(super) struct CardCommitment {
    pub schedule_id: Uuid,
    pub settlement_account_id: Option<Uuid>,
    pub due_date: NaiveDate,
    pub source: ScheduleSource,
    pub outstanding: i64,
    pub outstanding_at_horizon: i64,
}

/// Outstanding on the start date, with known later applications accounted for
/// separately so the forecast can include their actual postings just once.
pub(super) fn card_commitments(
    db: &Connection,
    from: NaiveDate,
    horizon: NaiveDate,
) -> Result<Vec<CardCommitment>, AppError> {
    rows(db,
        "SELECT s.id,c.settlement_account_id,s.due_date,s.source,
                s.principal_due-COALESCE(SUM(CASE WHEN a.effective_date<=?1 THEN a.amount ELSE 0 END),0) AS outstanding,
                s.principal_due-COALESCE(SUM(a.amount),0)
         FROM card_schedules s LEFT JOIN card_settings c ON c.card_id=s.card_id
         LEFT JOIN card_applications a ON a.schedule_id=s.id AND a.effective_date<=?2
         WHERE s.due_date<=?2 GROUP BY s.id HAVING outstanding>0
         ORDER BY s.due_date,s.id",
        params![from,horizon],|r| Ok(CardCommitment {
            schedule_id:id(r,0)?,settlement_account_id:optional_id(r,1)?,due_date:r.get(2)?,
            source:ScheduleSource::read(r,3)?,outstanding:r.get(4)?,outstanding_at_horizon:r.get(5)?,
        }),
    )
}

fn schedule_cap(
    db: &Connection,
    terms: &ScheduleInput,
    except: Option<Uuid>,
) -> Result<(), AppError> {
    let a = card(db, terms.card_id)?;
    active(&a)?;
    date(terms.due_date)?;
    positive_yen(terms.principal_due, "principalDue")?;
    let cap = match (&terms.purchase_event_id, &terms.opening_obligation) {
        (Some(purchase), None) => {
            let e = event(db, *purchase)?;
            if e.kind != EventKind::Expense
                || e.is_reversal
                || reversed(db, e.id)?
                || !e
                    .postings
                    .iter()
                    .any(|p| p.account_id == a.id && p.delta == -e.amount)
            {
                return Err(invalid(
                    "purchaseEventId",
                    "Select a posted purchase for this card.",
                ));
            }
            e.amount
        }
        (None, Some(reason)) => {
            label(reason, "openingObligation", MAX_DESCRIPTION_CHARS)?;
            let opening = latest_opening(db, a.id)?
                .filter(|o| o.amount < 0)
                .ok_or_else(|| blocked("Set a negative opening card balance first."))?;
            -opening.amount
        }
        _ => {
            return Err(invalid(
                "purchaseEventId",
                "Link a purchase or describe opening debt.",
            ));
        }
    };
    let used = amount(
        db,
        "SELECT COALESCE(SUM(principal_due),0) FROM card_schedules
        WHERE card_id=?1 AND purchase_event_id IS ?2 AND id IS NOT ?3",
        params![
            key(&a.id),
            terms.purchase_event_id.as_ref().map(key),
            except.as_ref().map(key)
        ],
    )?;
    if used + terms.principal_due > cap {
        return Err(invalid(
            "principalDue",
            "Schedules exceed the original principal.",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum FundingTarget {
    Schedule(Uuid),
    Bucket(Uuid),
}

impl FundingTarget {
    fn bindings(self) -> (Option<String>, Option<String>) {
        match self {
            Self::Schedule(id) => (Some(key(&id)), None),
            Self::Bucket(id) => (None, Some(key(&id))),
        }
    }
}

fn allocate(
    db: &Connection,
    input: &Funding,
    target: FundingTarget,
    reason: &str,
) -> Result<(), AppError> {
    date(input.as_of_date)?;
    yen(input.amount, "amount")?;
    if input.amount == 0 {
        return Err(invalid("amount", "Enter a nonzero allocation change."));
    }
    let account = account(db, input.account_id)?;
    if !account.kind.liquid() {
        return Err(invalid("accountId", "Select a liquid funding account."));
    }
    let (schedule, bucket) = target.bindings();
    if input.amount > 0 {
        active(&account)?;
        let actual = balance(db, &account, input.as_of_date, mapping_complete(db)?)?;
        let actual = actual
            .balance
            .filter(|_| actual.status == BalanceStatus::Complete)
            .ok_or_else(|| blocked("Complete the opening and cutover before funding."))?;
        let (_, peak) = funding_range(db, account.id, input.as_of_date, None, None)?;
        if actual - peak < input.amount {
            return Err(blocked("There is insufficient unallocated cash."));
        }
    } else {
        let (minimum, _) = funding_range(
            db,
            account.id,
            input.as_of_date,
            schedule.as_deref(),
            bucket.as_deref(),
        )?;
        if minimum + input.amount < 0 {
            return Err(blocked("Release exceeds funding on this account."));
        }
    }
    db.execute("INSERT INTO funding_allocations (id,account_id,schedule_id,bucket_id,amount,effective_date,reason)
        VALUES (?1,?2,?3,?4,?5,?6,?7)",params![key(&Uuid::new_v4()),key(&account.id),schedule,bucket,
        input.amount,input.as_of_date,reason])?;
    Ok(())
}

// A backdated change must also fit between subsequent funding and release rows.
fn funding_range(
    db: &Connection,
    account_id: Uuid,
    from: NaiveDate,
    schedule: Option<&str>,
    bucket: Option<&str>,
) -> Result<(i64, i64), AppError> {
    let (current, minimum, maximum): (i64, Option<i64>, Option<i64>) = db.query_row(
        "WITH chronology AS (
            SELECT effective_date, SUM(SUM(amount)) OVER (ORDER BY effective_date) AS running
            FROM funding_allocations
            WHERE account_id=?1 AND (?3 IS NULL OR schedule_id=?3) AND (?4 IS NULL OR bucket_id=?4)
            GROUP BY effective_date
        )
        SELECT COALESCE((SELECT running FROM chronology WHERE effective_date<=?2
                         ORDER BY effective_date DESC LIMIT 1),0), MIN(running), MAX(running)
        FROM chronology WHERE effective_date>?2",
        params![key(&account_id), from, schedule, bucket],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok((
        current.min(minimum.unwrap_or(current)),
        current.max(maximum.unwrap_or(current)),
    ))
}

fn bucket(db: &Connection, bucket_id: Uuid) -> Result<SavingsBucket, AppError> {
    Ok(db.query_row(
        "SELECT b.name,b.goal_id,b.archived_at,
        COALESCE((SELECT SUM(amount) FROM funding_allocations WHERE bucket_id=b.id),0)
        FROM savings_buckets b WHERE b.id=?1",
        [key(&bucket_id)],
        |r| {
            Ok(SavingsBucket {
                id: bucket_id,
                name: r.get(0)?,
                goal_id: optional_id(r, 1)?,
                archived_at: r.get(2)?,
                funded: r.get(3)?,
            })
        },
    )?)
}

impl Finance<'_> {
    pub fn set_card_settings(
        &self,
        request: Uuid,
        input: CardSettings,
    ) -> Result<WriteResponse<CardSettings>, AppError> {
        self.write(request,"set_card_settings",&input,|db| {
            active(&card(db,input.card_id)?)?;
            let bank=account(db,input.settlement_account_id)?;
            active(&bank)?;
            if bank.kind!=AccountKind::Bank { return Err(invalid("settlementAccountId","Select a bank account.")); }
            day_of_month(input.closing_day)?;
            day_of_month(input.payment_day)?;
            db.execute("INSERT INTO card_settings VALUES (?1,?2,?3,?4) ON CONFLICT(card_id) DO UPDATE
                SET settlement_account_id=excluded.settlement_account_id,closing_day=excluded.closing_day,payment_day=excluded.payment_day",
                params![key(&input.card_id),key(&bank.id),input.closing_day,input.payment_day])?;
            Ok(input.clone())
        })
    }

    pub fn card_settings(&self, card_id: Uuid) -> Result<Option<CardSettings>, AppError> {
        self.read(|db| {
            card(db,card_id)?;
            Ok(db.query_row("SELECT settlement_account_id,closing_day,payment_day FROM card_settings WHERE card_id=?1",
                [key(&card_id)],|r|Ok(CardSettings{card_id,settlement_account_id:id(r,0)?,closing_day:r.get(1)?,payment_day:r.get(2)?})).optional()?)
        })
    }

    pub fn create_schedule(
        &self,
        request: Uuid,
        terms: ScheduleInput,
    ) -> Result<WriteResponse<Schedule>, AppError> {
        self.write(request,"create_schedule",&terms,|db| {
            schedule_cap(db,&terms,None)?;
            let schedule_id=Uuid::new_v4();
            db.execute("INSERT INTO card_schedules (id,card_id,purchase_event_id,opening_obligation,due_date,principal_due,source)
                VALUES (?1,?2,?3,?4,?5,?6,?7)",params![key(&schedule_id),key(&terms.card_id),
                terms.purchase_event_id.as_ref().map(key),terms.opening_obligation,terms.due_date,terms.principal_due,terms.source.as_str()])?;
            schedule(db,schedule_id,None)
        })
    }

    pub fn revise_schedule(
        &self,
        request: Uuid,
        schedule_id: Uuid,
        due: NaiveDate,
        principal: i64,
        source: ScheduleSource,
    ) -> Result<WriteResponse<Schedule>, AppError> {
        self.write(
            request,
            "revise_schedule",
            &(schedule_id, due, principal, source),
            |db| {
                let prior = schedule(db, schedule_id, None)?;
                if prior.statement_id.is_some() {
                    return Err(blocked(
                        "A confirmed statement's schedules cannot be edited.",
                    ));
                }
                if principal < prior.paid + prior.refunded + prior.funded_reserve {
                    return Err(invalid(
                        "principalDue",
                        "Principal cannot be lower than payments, refunds and reserves.",
                    ));
                }
                let terms = ScheduleInput {
                    due_date: due,
                    principal_due: principal,
                    source,
                    ..prior.terms
                };
                schedule_cap(db, &terms, Some(schedule_id))?;
                db.execute(
                    "UPDATE card_schedules SET due_date=?2,principal_due=?3,source=?4 WHERE id=?1",
                    params![key(&schedule_id), due, principal, source.as_str()],
                )?;
                schedule(db, schedule_id, None)
            },
        )
    }

    pub fn card_schedules(&self, card_id: Uuid) -> Result<Vec<Schedule>, AppError> {
        self.read(|db| {
            card(db, card_id)?;
            rows(
                db,
                "SELECT id FROM card_schedules WHERE card_id=?1 ORDER BY due_date,id",
                [key(&card_id)],
                |r| id(r, 0),
            )?
            .into_iter()
            .map(|id| schedule(db, id, None))
            .collect()
        })
    }

    pub fn confirm_card_statement(
        &self,
        request: Uuid,
        terms: StatementInput,
    ) -> Result<WriteResponse<Statement>, AppError> {
        self.write(request, "confirm_card_statement", &terms, |db| {
            active(&card(db, terms.card_id)?)?;
            date(terms.closing_on)?;
            date(terms.due_date)?;
            yen(terms.principal_confirmed, "principalConfirmed")?;
            let unique: BTreeSet<_> = terms.schedule_ids.iter().collect();
            if terms.due_date <= terms.closing_on
                || unique.is_empty()
                || unique.len() != terms.schedule_ids.len()
            {
                return Err(invalid(
                    "scheduleIds",
                    "Select distinct schedules and a due date after closing.",
                ));
            }
            if exists(
                db,
                "SELECT 1 FROM card_statements WHERE card_id=?1 AND (closing_on=?2 OR due_date=?3)",
                params![key(&terms.card_id), terms.closing_on, terms.due_date],
            )? {
                return Err(blocked(
                    "This card already has a statement for that closing or due date.",
                ));
            }
            let mut principal = Vec::new();
            for &id in &terms.schedule_ids {
                let s = schedule(db, id, None)?;
                if s.statement_id.is_some()
                    || s.terms.card_id != terms.card_id
                    || s.terms.due_date != terms.due_date
                {
                    return Err(invalid(
                        "scheduleIds",
                        "Schedules must be unassigned and share the card and due date.",
                    ));
                }
                principal.push(s.terms.principal_due - s.refunded);
            }
            if total(principal)? != terms.principal_confirmed {
                return Err(invalid(
                    "principalConfirmed",
                    "The statement total differs from its schedules.",
                ));
            }
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO card_statements VALUES (?1,?2,?3,?4,?5)",
                params![
                    key(&id),
                    key(&terms.card_id),
                    terms.closing_on,
                    terms.due_date,
                    terms.principal_confirmed
                ],
            )?;
            for schedule_id in &terms.schedule_ids {
                db.execute(
                    "UPDATE card_schedules SET statement_id=?2,source='actual' WHERE id=?1",
                    params![key(schedule_id), key(&id)],
                )?;
            }
            Ok(Statement {
                id,
                terms: terms.clone(),
            })
        })
    }

    pub fn card_statements(&self, card_id: Uuid) -> Result<Vec<Statement>, AppError> {
        self.read(|db| {
            card(db, card_id)?;
            let mut statements = rows(
                db,
                "SELECT id, closing_on, due_date, principal_confirmed
                FROM card_statements WHERE card_id=?1 ORDER BY closing_on,id",
                [key(&card_id)],
                |r| {
                    Ok(Statement {
                        id: id(r, 0)?,
                        terms: StatementInput {
                            card_id,
                            closing_on: r.get(1)?,
                            due_date: r.get(2)?,
                            principal_confirmed: r.get(3)?,
                            schedule_ids: Vec::new(),
                        },
                    })
                },
            )?;
            for statement in &mut statements {
                statement.terms.schedule_ids = rows(
                    db,
                    "SELECT id FROM card_schedules
                    WHERE statement_id=?1 ORDER BY id",
                    [key(&statement.id)],
                    |r| id(r, 0),
                )?;
            }
            Ok(statements)
        })
    }

    pub fn apply_card_event(
        &self,
        request: Uuid,
        schedule_id: Uuid,
        event_id: Uuid,
        value: i64,
    ) -> Result<WriteResponse<Schedule>, AppError> {
        self.write(
            request,
            "apply_card_event",
            &(schedule_id, event_id, value),
            |db| {
                positive_yen(value, "amount")?;
                let s = schedule(db, schedule_id, None)?;
                let e = event(db, event_id)?;
                let kind = match e.kind {
                    EventKind::CardRepayment => "payment",
                    EventKind::Refund
                        if e.original_event_id == s.terms.purchase_event_id
                            && e.original_event_id.is_some() =>
                    {
                        "refund"
                    }
                    _ => {
                        return Err(invalid(
                            "eventId",
                            "Select a repayment or a refund for the scheduled purchase.",
                        ));
                    }
                };
                let p = e
                    .postings
                    .iter()
                    .find(|p| p.account_id == s.terms.card_id && p.delta == e.amount)
                    .ok_or_else(|| invalid("eventId", "The event does not credit this card."))?;
                if e.is_reversal || reversed(db, e.id)? {
                    return Err(blocked("A reversed event cannot settle a schedule."));
                }
                let old: Option<i64> = db
                    .query_row(
                        "SELECT amount FROM card_applications WHERE schedule_id=?1 AND event_id=?2",
                        params![key(&s.id), key(&e.id)],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(old) = old {
                    if old == value {
                        return Ok(s);
                    }
                    return Err(blocked(
                        "This event is already applied for a different amount.",
                    ));
                }
                let applied = amount(
                    db,
                    "SELECT COALESCE(SUM(amount),0) FROM card_applications WHERE event_id=?1",
                    [key(&e.id)],
                )?;
                if value > s.outstanding || applied + value > e.amount {
                    return Err(invalid(
                        "amount",
                        "Application exceeds the schedule or event amount.",
                    ));
                }
                db.execute(
                    "INSERT INTO card_applications VALUES (?1,?2,?3,?4,?5)",
                    params![key(&s.id), key(&e.id), kind, value, p.effective_date],
                )?;
                let reserves = rows(
                    db,
                    "SELECT account_id,SUM(amount),MAX(effective_date) FROM funding_allocations
                WHERE schedule_id=?1 GROUP BY account_id HAVING SUM(amount)>0 ORDER BY account_id",
                    [key(&s.id)],
                    |r| Ok((id(r, 0)?, r.get::<_, i64>(1)?, r.get::<_, NaiveDate>(2)?)),
                )?;
                let mut release = value.min(s.funded_reserve);
                for (account_id, assigned, last_date) in reserves {
                    let consumed = release.min(assigned);
                    if consumed == 0 {
                        break;
                    }
                    allocate(
                        db,
                        &Funding {
                            account_id,
                            amount: -consumed,
                            as_of_date: p.effective_date.max(last_date),
                        },
                        FundingTarget::Schedule(s.id),
                        kind,
                    )?;
                    release -= consumed;
                }
                schedule(db, s.id, None)
            },
        )
    }

    pub fn fund_card_reserve(
        &self,
        request: Uuid,
        schedule_id: Uuid,
        input: Funding,
    ) -> Result<WriteResponse<Schedule>, AppError> {
        self.write(request, "fund_card_reserve", &(schedule_id, &input), |db| {
            let s = schedule(db, schedule_id, None)?;
            if s.funded_reserve + input.amount > s.outstanding {
                return Err(invalid("amount", "Reserve exceeds outstanding principal."));
            }
            allocate(
                db,
                &input,
                FundingTarget::Schedule(schedule_id),
                "card reserve",
            )?;
            schedule(db, schedule_id, None)
        })
    }

    pub fn save_savings_goal(
        &self,
        request: Uuid,
        existing: Option<Uuid>,
        input: SavingsGoalInput,
        archive: bool,
    ) -> Result<WriteResponse<SavingsGoal>, AppError> {
        self.write(request,"save_savings_goal",&(existing,&input,archive),|db| {
            let name=label(&input.name,"name",MAX_NAME_CHARS)?;
            positive_yen(input.target,"target")?;
            if let Some(on)=input.target_date { date(on)?; }
            let goal_id=existing.unwrap_or_else(Uuid::new_v4);
            let archived_at=archive.then(now);
            let args=params![key(&goal_id),name,input.target,input.target_date,archived_at];
            if let Some(existing)=existing {
                if archive && exists(db,"SELECT 1 FROM savings_buckets WHERE goal_id=?1 AND archived_at IS NULL",[key(&existing)])? {
                    return Err(blocked("Archive or move active funding buckets first."));
                }
                if db.execute("UPDATE savings_goals SET name=?2,target=?3,target_date=?4,archived_at=?5 WHERE id=?1",args)?==0 {
                    return Err(missing());
                }
            } else {
                db.execute("INSERT INTO savings_goals VALUES (?1,?2,?3,?4,?5)",args)?;
            }
            Ok(SavingsGoal { id:goal_id,terms:SavingsGoalInput{name,..input.clone()},archived_at })
        })
    }

    pub fn savings_goals(&self) -> Result<Vec<SavingsGoal>, AppError> {
        self.read(|db| {
            rows(
                db,
                "SELECT id,name,target,target_date,archived_at FROM savings_goals ORDER BY name,id",
                [],
                |r| {
                    Ok(SavingsGoal {
                        id: id(r, 0)?,
                        terms: SavingsGoalInput {
                            name: r.get(1)?,
                            target: r.get(2)?,
                            target_date: r.get(3)?,
                        },
                        archived_at: r.get(4)?,
                    })
                },
            )
        })
    }

    pub fn save_savings_bucket(
        &self,
        request: Uuid,
        existing: Option<Uuid>,
        name: &str,
        goal_id: Option<Uuid>,
        archive: bool,
    ) -> Result<WriteResponse<SavingsBucket>, AppError> {
        self.write(
            request,
            "save_savings_bucket",
            &(existing, name, goal_id, archive),
            |db| {
                let name = label(name, "name", MAX_NAME_CHARS)?;
                if let Some(goal_id) = goal_id
                    && !exists(
                        db,
                        "SELECT 1 FROM savings_goals WHERE id=?1 AND archived_at IS NULL",
                        [key(&goal_id)],
                    )?
                {
                    return Err(missing());
                }
                let bucket_id = existing.unwrap_or_else(Uuid::new_v4);
                let stamp = archive.then(now);
                let args = params![key(&bucket_id), goal_id.as_ref().map(key), name, stamp];
                if let Some(existing) = existing {
                    if archive && bucket(db, existing)?.funded != 0 {
                        return Err(blocked("Release bucket funding before archiving."));
                    }
                    if db.execute(
                        "UPDATE savings_buckets SET goal_id=?2,name=?3,archived_at=?4 WHERE id=?1",
                        args,
                    )? == 0
                    {
                        return Err(missing());
                    }
                } else {
                    db.execute("INSERT INTO savings_buckets VALUES (?1,?2,?3,?4)", args)?;
                }
                bucket(db, bucket_id)
            },
        )
    }

    pub fn savings_buckets(&self) -> Result<Vec<SavingsBucket>, AppError> {
        self.read(|db| {
            rows(
                db,
                "SELECT id FROM savings_buckets ORDER BY name,id",
                [],
                |r| id(r, 0),
            )?
            .into_iter()
            .map(|id| bucket(db, id))
            .collect()
        })
    }

    pub fn fund_savings_bucket(
        &self,
        request: Uuid,
        bucket_id: Uuid,
        input: Funding,
    ) -> Result<WriteResponse<SavingsBucket>, AppError> {
        self.write(request, "fund_savings_bucket", &(bucket_id, &input), |db| {
            let b = bucket(db, bucket_id)?;
            if b.archived_at.is_some() {
                return Err(blocked("This funding bucket has been archived."));
            }
            allocate(
                db,
                &input,
                FundingTarget::Bucket(bucket_id),
                "savings bucket",
            )?;
            bucket(db, bucket_id)
        })
    }

    pub fn allocation_capacity(&self, on: NaiveDate) -> Result<Vec<AllocationCapacity>, AppError> {
        date(on)?;
        self.read(|db| {
            let mapping_complete = mapping_complete(db)?;
            accounts(db, true)?
                .into_iter()
                .filter(|a| a.kind.liquid())
                .map(|a| {
                    let balance = balance(db, &a, on, mapping_complete)?;
                    let funded = funded(db, a.id, Some(on))?;
                    let remaining = balance
                        .balance
                        .filter(|_| balance.status == BalanceStatus::Complete)
                        .map(|cash| total([cash, -funded]))
                        .transpose()?;
                    Ok(AllocationCapacity {
                        balance,
                        funded,
                        allocatable: remaining.map(|v| v.max(0)),
                        shortfall: remaining.map(|v| if funded > 0 { (-v).max(0) } else { 0 }),
                    })
                })
                .collect()
        })
    }
}
