use super::cards::{day_of_month, funded, schedule, schedule_funding, schedules_due};
use super::ledger::*;
use super::review::{Position, nonnegative, position};
use super::*;
use crate::commands::wire::MAX_NAME_CHARS;
use chrono::{Datelike, Days, Months};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_FORECAST_DAYS: i64 = 366;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSettings {
    pub time_zone: String,
    pub payday_day: u32,
    pub default_account_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub start: NaiveDate,
    pub end_exclusive: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBudget {
    pub category_id: Uuid,
    pub cycle_start: NaiveDate,
    pub planned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BudgetResult {
    pub id: Uuid,
    #[serde(flatten)]
    pub terms: CategoryBudget,
    pub category_name: String,
    pub gross_spending: i64,
    pub refunds: i64,
    pub net_spending: i64,
    pub remaining: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSummary {
    #[serde(flatten)]
    pub cycle: Cycle,
    pub budgets: Vec<BudgetResult>,
    pub unknown_purchase_date_count: usize,
    pub coverage: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Recurrence {
    #[default]
    None,
    Monthly,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    Confirmed,
    #[default]
    Estimated,
    ScenarioOnly,
}

impl Recurrence {
    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Monthly => "monthly",
        }
    }
}

impl Certainty {
    fn name(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Estimated => "estimated",
            Self::ScenarioOnly => "scenario_only",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedFlow {
    pub account_id: Uuid,
    pub on_date: NaiveDate,
    pub amount: i64,
    pub label: String,
    #[serde(default)]
    pub recurrence: Recurrence,
    #[serde(default)]
    pub certainty: Certainty,
    pub schedule_id: Option<Uuid>,
    pub budget_category_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub id: Uuid,
    #[serde(flatten)]
    pub terms: ExpectedFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlowOverride {
    pub flow_id: Uuid,
    pub scheduled_date: NaiveDate,
    pub override_date: Option<NaiveDate>,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlowMatch {
    pub flow_id: Uuid,
    pub scheduled_date: NaiveDate,
    pub event_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Movement {
    pub id: Uuid,
    pub amount: i64,
    pub source: String,
    pub label: String,
    pub certainty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForecastDay {
    pub account_id: Uuid,
    pub date: NaiveDate,
    pub start_balance: Option<i64>,
    pub end_balance: Option<i64>,
    pub movements: Vec<Movement>,
    pub intraday_order_uncertain: bool,
    pub shortfall: bool,
    pub coverage: BalanceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LowBalance {
    pub date: NaiveDate,
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub currency: String,
    pub scenario: String,
    pub days: Vec<ForecastDay>,
    pub lowest_by_account: BTreeMap<Uuid, LowBalance>,
    pub coverage: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAccount {
    pub account_id: Uuid,
    pub liquid_balance: Option<i64>,
    pub funded_earmarks: i64,
    pub unfunded_known_due: i64,
    pub raw_available: Option<i64>,
    pub spendable: Option<i64>,
    pub reserve_shortfall: i64,
    pub coverage: BalanceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub as_of_date: NaiveDate,
    pub horizon: NaiveDate,
    pub currency: String,
    pub accounts: Vec<AvailableAccount>,
    pub remaining_discretionary_budget: Option<i64>,
    pub coverage: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub as_of_date: NaiveDate,
    pub available_cash: Availability,
    pub living_budget: BudgetSummary,
    pub forecast: Forecast,
    pub financial_position: Position,
    pub monthly_actuals: Vec<MonthlyPoint>,
}

fn settings(db: &Connection) -> Result<(FinanceSettings, bool), AppError> {
    let zone = db.query_row(
        "SELECT time_zone FROM local_profile WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    let stored = db
        .query_row(
            "SELECT payday_day,default_account_id FROM finance_settings WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, optional_id(r, 1)?)),
        )
        .optional()?;
    let (payday_day, default_account_id) = stored.unwrap_or((25, None));
    Ok((
        FinanceSettings {
            time_zone: zone,
            payday_day,
            default_account_id,
        },
        stored.is_some(),
    ))
}

fn month_start(on: NaiveDate) -> NaiveDate {
    on.with_day(1).expect("valid month")
}

fn clamped_day(month: NaiveDate, day: u32) -> NaiveDate {
    (1..=day)
        .rev()
        .find_map(|d| month.with_day(d))
        .expect("validated day and month")
}

fn payday(db: &Connection, month: NaiveDate, day: u32) -> Result<NaiveDate, AppError> {
    Ok(db
        .query_row(
            "SELECT actual_date FROM payday_overrides WHERE month=?1",
            [month],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(|| clamped_day(month, day)))
}

fn cycle(db: &Connection, on: NaiveDate, day: u32) -> Result<Cycle, AppError> {
    date(on)?;
    let month = month_start(on);
    let this = payday(db, month, day)?;
    if on < this {
        let previous = month
            .checked_sub_months(Months::new(1))
            .ok_or_else(|| invalid("on", "Date is too early."))?;
        let start = payday(db, previous, day)?;
        date(start)?;
        Ok(Cycle {
            start,
            end_exclusive: this,
        })
    } else {
        let next = month
            .checked_add_months(Months::new(1))
            .ok_or_else(|| invalid("on", "Date is too late."))?;
        let end_exclusive = payday(db, next, day)?;
        date(end_exclusive)?;
        Ok(Cycle {
            start: this,
            end_exclusive,
        })
    }
}

fn spending(
    db: &Connection,
    category_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(i64, i64), AppError> {
    let (gross,refunds):(i64,i64)=db.query_row("SELECT
        COALESCE(SUM(CASE kind WHEN 'EXPENSE' THEN CASE WHEN is_reversal THEN -amount ELSE amount END ELSE 0 END),0),
        COALESCE(SUM(CASE kind WHEN 'REFUND' THEN CASE WHEN is_reversal THEN -amount ELSE amount END ELSE 0 END),0)
        FROM financial_events WHERE category_id=?1 AND occurred_on BETWEEN ?2 AND ?3",
        params![key(&category_id),from,to],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok((yen(gross, "grossSpending")?, yen(refunds, "refunds")?))
}

fn unknown_dates(db: &Connection) -> Result<usize, AppError> {
    Ok(db.query_row("SELECT COUNT(*) FROM financial_events e WHERE kind='EXPENSE' AND occurred_on IS NULL AND NOT is_reversal
        AND NOT EXISTS(SELECT 1 FROM financial_events r WHERE r.reverses_event_id=e.id)",[],|r|r.get(0))?)
}

fn budgets(
    db: &Connection,
    on: NaiveDate,
    through: Option<NaiveDate>,
) -> Result<BudgetSummary, AppError> {
    let cycle = cycle(db, on, settings(db)?.0.payday_day)?;
    let last = through
        .unwrap_or(cycle.end_exclusive.pred_opt().expect("cycle end"))
        .min(cycle.end_exclusive.pred_opt().expect("cycle end"));
    let definitions = rows(
        db,
        "SELECT b.id,b.category_id,b.planned,c.name FROM category_budgets b
        JOIN categories c ON c.id=b.category_id WHERE cycle_start=?1 ORDER BY c.name,b.id",
        [cycle.start],
        |r| {
            Ok((
                id(r, 0)?,
                id(r, 1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    )?;
    let budgets = definitions
        .into_iter()
        .map(|(id, category_id, planned, category_name)| {
            let (gross, refunds) = spending(db, category_id, cycle.start, last)?;
            let net_spending = total([gross, -refunds])?;
            Ok(BudgetResult {
                id,
                terms: CategoryBudget {
                    category_id,
                    cycle_start: cycle.start,
                    planned,
                },
                category_name,
                gross_spending: gross,
                refunds,
                net_spending,
                remaining: total([planned, -net_spending])?,
            })
        })
        .collect::<Result<_, AppError>>()?;
    let unknown_purchase_date_count = unknown_dates(db)?;
    Ok(BudgetSummary {
        cycle,
        budgets,
        unknown_purchase_date_count,
        coverage: if unknown_purchase_date_count == 0 {
            "known_dates_only"
        } else {
            "incomplete"
        }
        .into(),
    })
}

fn flow_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Flow> {
    let recurrence = if r.get::<_, String>(5)? == "monthly" {
        Recurrence::Monthly
    } else {
        Recurrence::None
    };
    let certainty = match r.get::<_, String>(6)?.as_str() {
        "confirmed" => Certainty::Confirmed,
        "estimated" => Certainty::Estimated,
        "scenario_only" => Certainty::ScenarioOnly,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(Flow {
        id: id(r, 0)?,
        terms: ExpectedFlow {
            account_id: id(r, 1)?,
            on_date: r.get(2)?,
            amount: r.get(3)?,
            label: r.get(4)?,
            recurrence,
            certainty,
            schedule_id: optional_id(r, 7)?,
            budget_category_id: optional_id(r, 8)?,
        },
    })
}

fn flow_list(db: &Connection) -> Result<Vec<Flow>, AppError> {
    rows(db,"SELECT id,account_id,on_date,amount,label,recurrence,certainty,schedule_id,budget_category_id
        FROM expected_flows ORDER BY on_date,id",[],flow_row)
}

fn flow(db: &Connection, flow_id: Uuid) -> Result<Flow, AppError> {
    Ok(db.query_row("SELECT id,account_id,on_date,amount,label,recurrence,certainty,schedule_id,budget_category_id
        FROM expected_flows WHERE id=?1",[key(&flow_id)],flow_row)?)
}

fn scheduled(input: &ExpectedFlow, on: NaiveDate) -> bool {
    on >= input.on_date
        && match input.recurrence {
            Recurrence::None => on == input.on_date,
            Recurrence::Monthly => on == clamped_day(month_start(on), input.on_date.day()),
        }
}

fn is_match(db: &Connection, flow_id: Uuid, on: NaiveDate) -> Result<Option<Uuid>, AppError> {
    Ok(db
        .query_row(
            "SELECT event_id FROM flow_matches WHERE flow_id=?1 AND scheduled_date=?2",
            params![key(&flow_id), on],
            |r| id(r, 0),
        )
        .optional()?)
}

#[derive(Clone, Copy)]
struct Occurrence {
    on: NaiveDate,
}

fn occurrences(
    db: &Connection,
    flow: &Flow,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<Occurrence>, AppError> {
    let overrides: BTreeMap<_, _> = rows(
        db,
        "SELECT scheduled_date,override_date,skipped FROM flow_overrides WHERE flow_id=?1
        AND (scheduled_date BETWEEN ?2 AND ?3 OR override_date BETWEEN ?2 AND ?3)",
        params![key(&flow.id), from, to],
        |r| {
            Ok((
                r.get::<_, NaiveDate>(0)?,
                (r.get::<_, Option<NaiveDate>>(1)?, r.get::<_, bool>(2)?),
            ))
        },
    )?
    .into_iter()
    .collect();
    let mut dates = std::collections::BTreeSet::new();
    match flow.terms.recurrence {
        Recurrence::None if (from..=to).contains(&flow.terms.on_date) => {
            dates.insert(flow.terms.on_date);
        }
        Recurrence::None => {}
        Recurrence::Monthly => {
            let mut month = month_start(from.max(flow.terms.on_date));
            while month <= to {
                let on = clamped_day(month, flow.terms.on_date.day());
                if on >= flow.terms.on_date && (from..=to).contains(&on) {
                    dates.insert(on);
                }
                month = month
                    .checked_add_months(Months::new(1))
                    .ok_or_else(|| invalid("to", "Date range is too large."))?;
            }
        }
    }
    dates.extend(overrides.keys().copied());
    let mut result = Vec::new();
    for scheduled in dates {
        let (replacement, skipped) = overrides.get(&scheduled).copied().unwrap_or_default();
        let on = replacement.unwrap_or(scheduled);
        if !skipped && (from..=to).contains(&on) && is_match(db, flow.id, scheduled)?.is_none() {
            result.push(Occurrence { on });
        }
    }
    Ok(result)
}

fn cash_accounts(db: &Connection) -> Result<Vec<Account>, AppError> {
    Ok(accounts(db, true)?
        .into_iter()
        .filter(|a| a.kind.liquid())
        .collect())
}

fn bills(
    db: &Connection,
    flows: &[Flow],
    category_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<i64, AppError> {
    let mut charges = Vec::new();
    for flow in flows.iter().filter(|f| {
        f.terms.budget_category_id == Some(category_id)
            && f.terms.amount < 0
            && f.terms.schedule_id.is_none()
            && f.terms.certainty == Certainty::Confirmed
    }) {
        charges.extend(
            occurrences(db, flow, from, to)?
                .iter()
                .map(|_| -flow.terms.amount),
        );
    }
    total(charges)
}

type Movements = BTreeMap<(Uuid, NaiveDate), Vec<Movement>>;

fn expected_movements(
    db: &Connection,
    flows: &[Flow],
    from: NaiveDate,
    to: NaiveDate,
    scenario: bool,
) -> Result<Movements, AppError> {
    let mut movements = Movements::new();
    for flow in flows {
        let terms = &flow.terms;
        if terms.schedule_id.is_some() || (terms.certainty == Certainty::ScenarioOnly && !scenario)
        {
            continue;
        }
        for occurrence in occurrences(db, flow, from, to)? {
            movements
                .entry((terms.account_id, occurrence.on))
                .or_default()
                .push(Movement {
                    id: flow.id,
                    amount: terms.amount,
                    source: "expected".into(),
                    label: terms.label.clone(),
                    certainty: terms.certainty.name().into(),
                });
        }
    }
    Ok(movements)
}

fn budget_movements(
    db: &Connection,
    flows: &[Flow],
    from: NaiveDate,
    to: NaiveDate,
    day: u32,
    movements: &mut Movements,
) -> Result<bool, AppError> {
    let start = cycle(db, from, day)?.start;
    let definitions=rows(db,"SELECT id,category_id,cycle_start,planned,payment_account_id FROM category_budgets
        WHERE payment_account_id IS NOT NULL AND cycle_start BETWEEN ?1 AND ?2 ORDER BY cycle_start,id",
        params![start,to],|r|Ok((id(r,0)?,id(r,1)?,r.get::<_,NaiveDate>(2)?,r.get::<_,i64>(3)?,id(r,4)?)))?;
    let mut card_timing_unknown = false;
    for (budget, category, start, planned, account_id) in definitions {
        let interval = cycle(db, start, day)?;
        if interval.start != start {
            continue;
        }
        let first = from.max(start);
        let end = interval.end_exclusive;
        let end_day = end.pred_opt().expect("cycle end");
        let last = to.min(end_day);
        if last < first {
            continue;
        }
        let (gross, refunds) = spending(db, category, start, end_day)?;
        let confirmed = bills(db, flows, category, first, end_day)?;
        let remaining = total([planned, -gross, refunds, -confirmed])?.max(0);
        if remaining == 0 {
            continue;
        }
        if !account(db, account_id)?.kind.liquid() {
            card_timing_unknown = true;
            continue;
        }
        let count = (end - first).num_days();
        for offset in 0..=(last - first).num_days() {
            let amount = remaining / count + i64::from(offset < remaining % count);
            if amount == 0 {
                continue;
            }
            let on = first
                .checked_add_days(Days::new(offset as u64))
                .expect("bounded offset");
            movements
                .entry((account_id, on))
                .or_default()
                .push(Movement {
                    id: budget,
                    amount: -amount,
                    source: "unspent_budget_assumption".into(),
                    label: "Unspent category budget".into(),
                    certainty: "scenario_assumption".into(),
                });
        }
    }
    Ok(card_timing_unknown)
}

fn forecast(
    db: &Connection,
    from: NaiveDate,
    to: NaiveDate,
    scenario: bool,
) -> Result<Forecast, AppError> {
    range(from, to, MAX_FORECAST_DAYS)?;
    let (cfg, configured) = settings(db)?;
    let all = cash_accounts(db)?;
    let flows = flow_list(db)?;
    let mut movements = expected_movements(db, &flows, from, to, scenario)?;
    let card_timing = budget_movements(db, &flows, from, to, cfg.payday_day, &mut movements)?;
    let mut missing_bank = false;
    for (id, bank) in schedules_due(db, from, to)? {
        let s = schedule(db, id, Some(to))?;
        if s.outstanding == 0 {
            continue;
        }
        if let Some(account) = bank {
            movements
                .entry((account, s.terms.due_date))
                .or_default()
                .push(Movement {
                    id,
                    amount: -s.outstanding,
                    source: "card_schedule".into(),
                    label: "Card payment".into(),
                    certainty: if s.terms.source == super::cards::ScheduleSource::Actual {
                        "actual"
                    } else {
                        "estimate"
                    }
                    .into(),
                });
        } else {
            missing_bank = true;
        }
    }
    let actual=rows(db,"SELECT p.account_id,p.effective_date,p.delta,e.id,e.kind FROM account_postings p
        JOIN financial_events e ON e.id=p.event_id WHERE p.effective_date>?1 AND p.effective_date<=?2
        ORDER BY p.effective_date,e.recorded_at,p.id",params![from,to],
        |r|Ok((id(r,0)?,r.get::<_,NaiveDate>(1)?,r.get::<_,i64>(2)?,id(r,3)?,r.get::<_,String>(4)?)))?;
    for (account, on, amount, id, label) in actual {
        movements.entry((account, on)).or_default().push(Movement {
            id,
            amount,
            source: "actual".into(),
            label,
            certainty: "posted".into(),
        });
    }
    let mut states = all
        .into_iter()
        .map(|a| balance(db, &a, from).map(|b| (a.id, b.balance, b.status)))
        .collect::<Result<Vec<_>, _>>()?;
    let missing_opening = states.is_empty()
        || states
            .iter()
            .any(|(_, _, status)| *status != BalanceStatus::Complete);
    let mut days = Vec::new();
    let mut lowest_by_account = BTreeMap::<Uuid, LowBalance>::new();
    for on in from.iter_days().take((to - from).num_days() as usize + 1) {
        for (account, running, coverage) in &mut states {
            let items = movements.remove(&(*account, on)).unwrap_or_default();
            let start_balance = *running;
            let end_balance = start_balance
                .map(|v| total(std::iter::once(v).chain(items.iter().map(|m| m.amount))))
                .transpose()?;
            let uncertain =
                items.iter().any(|i| i.amount > 0) && items.iter().any(|i| i.amount < 0);
            if let Some(value) = end_balance {
                let low = lowest_by_account.entry(*account).or_insert(LowBalance {
                    date: on,
                    balance: value,
                });
                if value < low.balance {
                    *low = LowBalance {
                        date: on,
                        balance: value,
                    };
                }
            }
            days.push(ForecastDay {
                account_id: *account,
                date: on,
                start_balance,
                end_balance,
                movements: items,
                intraday_order_uncertain: uncertain,
                shortfall: end_balance.is_some_and(|v| v < 0),
                coverage: *coverage,
            });
            *running = end_balance;
        }
    }
    let mut warnings = Vec::new();
    if missing_bank {
        warnings.push("Missing card settlement mapping".into());
    }
    if missing_opening {
        warnings.push("Missing or incomplete account opening or cutover".into());
    }
    if !configured {
        warnings.push("Finance settings are unconfirmed".into());
    }
    if card_timing {
        warnings.push("Credit-budget purchase timing is unknown until a card statement".into());
    }
    if days.iter().any(|day| day.intraday_order_uncertain) {
        warnings.push("Same-day clearing order is unknown".into());
    }
    Ok(Forecast {
        from,
        to,
        currency: "JPY".into(),
        scenario: if scenario {
            "with_scenario"
        } else {
            "baseline"
        }
        .into(),
        days,
        lowest_by_account,
        coverage: if missing_bank || missing_opening || !configured {
            "incomplete"
        } else {
            "partial_assumptions"
        }
        .into(),
        warnings,
    })
}

fn availability(
    db: &Connection,
    on: NaiveDate,
    horizon: NaiveDate,
) -> Result<Availability, AppError> {
    range(on, horizon, MAX_FORECAST_DAYS)?;
    let (_, configured) = settings(db)?;
    let all = cash_accounts(db)?;
    let mut missing_bank = false;
    let mut scheduled = BTreeMap::<Uuid, Vec<i64>>::new();
    for (id, bank) in schedules_due(db, on, horizon)? {
        let s = schedule(db, id, Some(on))?;
        if s.outstanding == 0 {
            continue;
        }
        if let Some(bank) = bank {
            let reserved = schedule_funding(db, id, Some(bank), Some(on))?;
            scheduled
                .entry(bank)
                .or_default()
                .push((s.outstanding - reserved).max(0));
        } else {
            missing_bank = true;
        }
    }
    let flows = flow_list(db)?;
    let mut bills_due = BTreeMap::<Uuid, Vec<i64>>::new();
    for f in flows.iter().filter(|f| {
        f.terms.schedule_id.is_none()
            && f.terms.certainty == Certainty::Confirmed
            && f.terms.amount < 0
    }) {
        bills_due.entry(f.terms.account_id).or_default().extend(
            occurrences(db, f, on, horizon)?
                .iter()
                .map(|_| -f.terms.amount),
        );
    }
    let summary = budgets(db, on, Some(on))?;
    // A matched future posting replaces the planned bill and still consumes
    // available cash once. A dated expense may already count in the budget.
    let matched_bills = rows(
        db,
        "SELECT f.account_id, f.budget_category_id, -p.delta, p.effective_date,
                COALESCE(e.kind='EXPENSE' AND e.category_id=f.budget_category_id
                    AND e.occurred_on BETWEEN ?2 AND ?1,0)
         FROM flow_matches m JOIN expected_flows f ON f.id=m.flow_id
         JOIN financial_events e ON e.id=m.event_id
         JOIN account_postings p ON p.event_id=e.id AND p.account_id=f.account_id
         WHERE f.schedule_id IS NULL AND f.certainty='confirmed' AND p.delta<0
         AND p.effective_date>?1 AND p.effective_date<=?3",
        params![
            on,
            summary.cycle.start,
            horizon.max(summary.cycle.end_exclusive)
        ],
        |r| {
            Ok((
                id(r, 0)?,
                optional_id(r, 1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, NaiveDate>(3)?,
                r.get::<_, bool>(4)?,
            ))
        },
    )?;
    for &(account_id, _, amount, effective, _) in &matched_bills {
        if effective <= horizon {
            bills_due.entry(account_id).or_default().push(amount);
        }
    }
    let mut accounts = Vec::new();
    for a in all {
        let b = balance(db, &a, on)?;
        let earmarks = funded(db, a.id, Some(on))?;
        let reserve_shortfall = total(scheduled.remove(&a.id).unwrap_or_default())?;
        let due = total([
            reserve_shortfall,
            total(bills_due.remove(&a.id).unwrap_or_default())?,
        ])?;
        let raw_available = b
            .balance
            .filter(|_| b.status == BalanceStatus::Complete)
            .map(|v| total([v, -earmarks, -due]))
            .transpose()?;
        accounts.push(AvailableAccount {
            account_id: a.id,
            liquid_balance: b.balance,
            funded_earmarks: earmarks,
            unfunded_known_due: due,
            raw_available,
            spendable: raw_available.map(|a| a.max(0)),
            reserve_shortfall,
            coverage: b.status,
        });
    }
    let mut discretionary = Vec::new();
    for b in &summary.budgets {
        let known = bills(
            db,
            &flows,
            b.terms.category_id,
            on,
            summary.cycle.end_exclusive.pred_opt().expect("cycle end"),
        )?;
        let matched = matched_bills
            .iter()
            .filter(|(_, category, _, effective, budgeted)| {
                !budgeted
                    && *category == Some(b.terms.category_id)
                    && *effective < summary.cycle.end_exclusive
            })
            .map(|(_, _, amount, _, _)| -*amount);
        discretionary.push(total([b.remaining, -known].into_iter().chain(matched))?.max(0));
    }
    let incomplete_openings = accounts.is_empty()
        || accounts
            .iter()
            .any(|b| b.coverage != BalanceStatus::Complete);
    let unknown = summary.unknown_purchase_date_count > 0;
    let mut warnings = Vec::new();
    if !configured {
        warnings.push("Finance settings are unconfirmed".into());
    }
    if unknown {
        warnings.push("Some purchase dates are unknown".into());
    }
    if missing_bank {
        warnings.push("Card settlement bank is missing".into());
    }
    if incomplete_openings {
        warnings.push("Missing or incomplete account opening or cutover".into());
    }
    Ok(Availability {
        as_of_date: on,
        horizon,
        currency: "JPY".into(),
        accounts,
        remaining_discretionary_budget: if unknown {
            None
        } else {
            Some(total(discretionary)?)
        },
        coverage: if warnings.is_empty() {
            "complete"
        } else {
            "incomplete"
        }
        .into(),
        warnings,
    })
}

impl Finance<'_> {
    pub fn finance_settings(&self) -> Result<FinanceSettings, AppError> {
        self.read(|db| settings(db).map(|s| s.0))
    }

    pub fn set_finance_settings(
        &self,
        request: Uuid,
        input: FinanceSettings,
    ) -> Result<WriteResponse<FinanceSettings>, AppError> {
        self.write(request, "set_finance_settings", &input, |db| {
            let zone: Tz = input
                .time_zone
                .parse()
                .map_err(|_| invalid("timeZone", "Select a valid IANA time zone."))?;
            day_of_month(input.payday_day)?;
            if let Some(id) = input.default_account_id {
                active(&account(db, id)?)?;
            }
            db.execute(
                "UPDATE local_profile SET time_zone=?1 WHERE singleton=1",
                [zone.name()],
            )?;
            db.execute(
                "INSERT INTO finance_settings VALUES (1,?1,?2) ON CONFLICT(singleton) DO UPDATE
                SET payday_day=excluded.payday_day,default_account_id=excluded.default_account_id",
                params![input.payday_day, input.default_account_id.as_ref().map(key)],
            )?;
            settings(db).map(|s| s.0)
        })
    }

    pub fn set_payday_override(
        &self,
        request: Uuid,
        month: NaiveDate,
        on: NaiveDate,
    ) -> Result<WriteResponse<NaiveDate>, AppError> {
        self.write(request,"set_payday_override",&(month,on),|db| {
            date(on)?;
            if month_start(on)!=month { return Err(invalid("month","Override must fall within its named month.")); }
            db.execute("INSERT INTO payday_overrides VALUES (?1,?2) ON CONFLICT(month) DO UPDATE SET actual_date=excluded.actual_date",
                params![month,on])?;
            Ok(on)
        })
    }

    pub fn budget_summary(&self, on: NaiveDate) -> Result<BudgetSummary, AppError> {
        self.read(|db| budgets(db, on, None))
    }

    pub fn set_category_budget(
        &self,
        request: Uuid,
        input: CategoryBudget,
    ) -> Result<WriteResponse<BudgetResult>, AppError> {
        self.write(request,"set_category_budget",&input,|db| {
            nonnegative(input.planned,"planned")?;
            let category=category(db,input.category_id)?;
            if category.kind!=CategoryType::Expense || category.archived_at.is_some() {
                return Err(invalid("categoryId","Select an active expense category."));
            }
            if cycle(db,input.cycle_start,settings(db)?.0.payday_day)?.start!=input.cycle_start {
                return Err(invalid("cycleStart","Cycle start must be a configured payday."));
            }
            db.execute("INSERT INTO category_budgets (id,category_id,cycle_start,planned) VALUES (?1,?2,?3,?4)
                ON CONFLICT(category_id,cycle_start) DO UPDATE SET planned=excluded.planned",
                params![key(&Uuid::new_v4()),key(&input.category_id),input.cycle_start,input.planned])?;
            budgets(db,input.cycle_start,None)?.budgets.into_iter().find(|b|b.terms.category_id==input.category_id).ok_or_else(missing)
        })
    }

    pub fn set_budget_payment_account(
        &self,
        request: Uuid,
        category_id: Uuid,
        start: NaiveDate,
        account_id: Option<Uuid>,
    ) -> Result<WriteResponse<Option<Uuid>>, AppError> {
        self.write(request,"set_budget_payment_account",&(category_id,start,account_id),|db| {
            date(start)?;
            if let Some(id)=account_id {
                let a=account(db,id)?;
                active(&a)?;
                if a.kind==AccountKind::BrokerageCash { return Err(invalid("accountId","Select cash, bank, or card.")); }
            }
            if db.execute("UPDATE category_budgets SET payment_account_id=?3 WHERE category_id=?1 AND cycle_start=?2",
                params![key(&category_id),start,account_id.as_ref().map(key)])?==0 { return Err(missing()); }
            Ok(account_id)
        })
    }

    pub fn create_expected_flow(
        &self,
        request: Uuid,
        terms: ExpectedFlow,
    ) -> Result<WriteResponse<Flow>, AppError> {
        self.write(request, "create_expected_flow", &terms, |db| {
            date(terms.on_date)?;
            yen(terms.amount, "amount")?;
            let a = account(db, terms.account_id)?;
            active(&a)?;
            if !a.kind.liquid() || terms.amount == 0 {
                return Err(invalid(
                    "amount",
                    "Enter a nonzero movement for a liquid account.",
                ));
            }
            if let Some(schedule_id) = terms.schedule_id {
                schedule(db, schedule_id, None)?;
                if terms.amount >= 0 || terms.recurrence != Recurrence::None {
                    return Err(invalid(
                        "scheduleId",
                        "Use a single outflow for a schedule.",
                    ));
                }
                if exists(
                    db,
                    "SELECT 1 FROM expected_flows WHERE schedule_id=?1",
                    [key(&schedule_id)],
                )? {
                    return Err(blocked(
                        "An expected flow already represents this schedule.",
                    ));
                }
            }
            if let Some(category_id) = terms.budget_category_id
                && (category(db, category_id)?.kind != CategoryType::Expense || terms.amount >= 0)
            {
                return Err(invalid(
                    "budgetCategoryId",
                    "Link an outgoing flow to an expense category.",
                ));
            }
            let label = label(&terms.label, "label", MAX_NAME_CHARS)?;
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO expected_flows VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    key(&id),
                    key(&a.id),
                    terms.on_date,
                    terms.amount,
                    label,
                    terms.recurrence.name(),
                    terms.certainty.name(),
                    terms.schedule_id.as_ref().map(key),
                    terms.budget_category_id.as_ref().map(key)
                ],
            )?;
            Ok(Flow {
                id,
                terms: ExpectedFlow {
                    label,
                    ..terms.clone()
                },
            })
        })
    }

    pub fn expected_flows(&self) -> Result<Vec<Flow>, AppError> {
        self.read(flow_list)
    }

    pub fn override_flow(
        &self,
        request: Uuid,
        input: FlowOverride,
    ) -> Result<WriteResponse<FlowOverride>, AppError> {
        self.write(request,"override_flow",&input,|db| {
            date(input.scheduled_date)?;
            if let Some(on)=input.override_date { date(on)?; }
            if !scheduled(&flow(db,input.flow_id)?.terms,input.scheduled_date) || (input.skipped && input.override_date.is_some()) {
                return Err(invalid("scheduledDate","Select a scheduled occurrence and either skip it or reschedule it."));
            }
            if is_match(db,input.flow_id,input.scheduled_date)?.is_some() { return Err(blocked("Unmatch the actual before changing the occurrence.")); }
            db.execute("INSERT INTO flow_overrides VALUES (?1,?2,?3,?4) ON CONFLICT(flow_id,scheduled_date) DO UPDATE
                SET override_date=excluded.override_date,skipped=excluded.skipped",
                params![key(&input.flow_id),input.scheduled_date,input.override_date,input.skipped])?;
            Ok(input.clone())
        })
    }

    pub fn match_flow(
        &self,
        request: Uuid,
        input: FlowMatch,
    ) -> Result<WriteResponse<FlowMatch>, AppError> {
        self.write(request, "match_flow", &input, |db| {
            date(input.scheduled_date)?;
            let f = flow(db, input.flow_id)?;
            if !scheduled(&f.terms, input.scheduled_date) {
                return Err(invalid("scheduledDate", "Select a scheduled occurrence."));
            }
            if let Some(previous) = is_match(db, input.flow_id, input.scheduled_date)? {
                if previous == input.event_id {
                    return Ok(input.clone());
                }
                return Err(blocked("The occurrence already matches another event."));
            }
            if exists(
                db,
                "SELECT 1 FROM flow_overrides WHERE flow_id=?1 AND scheduled_date=?2 AND skipped",
                params![key(&input.flow_id), input.scheduled_date],
            )? {
                return Err(blocked("Restore the skipped occurrence before matching."));
            }
            let e = event(db, input.event_id)?;
            if e.is_reversal
                || reversed(db, e.id)?
                || !e
                    .postings
                    .iter()
                    .any(|p| p.account_id == f.terms.account_id && p.delta == f.terms.amount)
            {
                return Err(invalid(
                    "eventId",
                    "Select a posted movement of the same signed amount and account.",
                ));
            }
            if exists(
                db,
                "SELECT 1 FROM flow_matches WHERE event_id=?1",
                [key(&e.id)],
            )? {
                return Err(blocked("This event already matches another occurrence."));
            }
            db.execute(
                "INSERT INTO flow_matches VALUES (?1,?2,?3)",
                params![key(&input.flow_id), input.scheduled_date, key(&e.id)],
            )?;
            Ok(input.clone())
        })
    }

    pub fn unmatch_flow(
        &self,
        request: Uuid,
        input: FlowMatch,
    ) -> Result<WriteResponse<FlowMatch>, AppError> {
        self.write(request, "unmatch_flow", &input, |db| {
            if db.execute(
                "DELETE FROM flow_matches WHERE flow_id=?1 AND scheduled_date=?2 AND event_id=?3",
                params![
                    key(&input.flow_id),
                    input.scheduled_date,
                    key(&input.event_id)
                ],
            )? == 0
            {
                return Err(missing());
            }
            Ok(input.clone())
        })
    }

    pub fn cash_forecast(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        scenario: bool,
    ) -> Result<Forecast, AppError> {
        self.read(|db| forecast(db, from, to, scenario))
    }

    pub fn available_cash(
        &self,
        on: NaiveDate,
        horizon: NaiveDate,
    ) -> Result<Availability, AppError> {
        self.read(|db| availability(db, on, horizon))
    }

    pub fn dashboard(&self, on: NaiveDate) -> Result<Dashboard, AppError> {
        self.read(|db| {
            let living_budget = budgets(db, on, None)?;
            let horizon = living_budget.cycle.end_exclusive;
            Ok(Dashboard {
                as_of_date: on,
                available_cash: availability(db, on, horizon)?,
                living_budget,
                forecast: forecast(db, on, horizon, false)?,
                financial_position: position(db, on)?,
                monthly_actuals: monthly(db, Some(month_start(on)), Some(on))?,
            })
        })
    }
}
