/**
 * Typed wrappers for the screens addendum (docs/contracts/v1-screens-addendum.md).
 * Writes take a `Write<T>` built once per user intent (`newWrite`) and resent
 * unchanged on retry; reads use camelCase named arguments. Rust computes every
 * financial figure; nothing here derives balances, totals or forecasts.
 */
import { runCommand } from './commands';
import type {
  Account,
  AllocationCapacity,
  Availability,
  BudgetResult,
  BudgetSummary,
  CanonicalCategoryTotal,
  CardSettings,
  Category,
  CategoryBudget,
  Cutover,
  CutoverInput,
  CutoverPreview,
  Dashboard,
  DateOnly,
  EventInput,
  ExpectedFlow,
  FinanceSettings,
  FinancialEvent,
  Flow,
  FlowMatch,
  FlowOverride,
  Forecast,
  Funding,
  HoldingsChange,
  Id,
  LegacyAccount,
  LegacyMappings,
  LegacyRecords,
  LegacyReports,
  OpeningRevision,
  Position,
  Provenance,
  QualityIssue,
  Reconciliation,
  ReconciliationAudit,
  ReconciliationInput,
  Resolution,
  ResolutionPreview,
  SavingsBucket,
  SavingsGoal,
  Schedule,
  ScheduleInput,
  ScheduleSource,
  Statement,
  StatementInput,
  Valuation,
  ValuationInput,
  Write,
  WriteResult,
  Yen,
} from './types';

const write =
  <T, R>(command: string) =>
  (request: Write<T>) =>
    runCommand<WriteResult<R>>(command, { request });

type Range = { from?: DateOnly; to?: DateOnly };

// #7 ledger (beyond v1)
export const updateCategory = write<
  { categoryId: Id; name: string; type: Category['type']; parentId?: Id },
  Category
>('update_category');
export const deleteCategory = write<{ categoryId: Id }, Id>('delete_category');
export const archiveCategory = write<{ categoryId: Id; archived: boolean }, Category>('archive_category');
export const archiveAccount = write<{ accountId: Id; archived: boolean }, Account>('archive_account');
export const listOpeningRevisions = (accountId: Id) =>
  runCommand<OpeningRevision[]>('list_opening_revisions', { accountId });
export const reverseFinancialEvent = write<{ eventId: Id; reason: string }, FinancialEvent>(
  'reverse_financial_event',
);
export const replaceFinancialEvent = write<
  { eventId: Id; reason: string; replacement: EventInput },
  FinancialEvent
>('replace_financial_event');
export const getFinancialEvent = (eventId: Id) =>
  runCommand<FinancialEvent>('get_financial_event', { eventId });
export const getCanonicalByCategory = (range: Range = {}) =>
  runCommand<CanonicalCategoryTotal[]>('get_canonical_by_category', range);

// #8 earlier history and cutover
export const listLegacyRecords = () => runCommand<LegacyRecords>('list_legacy_records');
export const getLegacyReports = () => runCommand<LegacyReports>('get_legacy_reports');
export const getLegacyByCategory = (range: Range = {}) =>
  runCommand<{ categoryId: Id; categoryName: string; total: Yen }[]>('get_legacy_by_category', range);
export const getLegacyBalance = (accountType: LegacyAccount, asOfDate: DateOnly) =>
  runCommand<Yen | null>('get_legacy_balance', { accountType, asOfDate });
export const previewCutover = (input: CutoverInput) =>
  runCommand<CutoverPreview>('preview_cutover', { input });
export const confirmCutover = write<CutoverInput, Cutover>('confirm_cutover');
export const listLegacyProvenance = (unresolvedOnly = false) =>
  runCommand<Provenance[]>('list_legacy_provenance', { unresolvedOnly });
export const previewLegacyResolution = (mappings: LegacyMappings) =>
  runCommand<ResolutionPreview>('preview_legacy_resolution', { mappings });
export const confirmLegacyResolution = write<LegacyMappings, Resolution>('confirm_legacy_resolution');

// #9 cards, reserves, savings
export const getCardSettings = (cardId: Id) =>
  runCommand<CardSettings | null>('get_card_settings', { cardId });
export const setCardSettings = write<CardSettings, CardSettings>('set_card_settings');
export const listCardSchedules = (cardId: Id) => runCommand<Schedule[]>('list_card_schedules', { cardId });
export const createCardSchedule = write<ScheduleInput, Schedule>('create_card_schedule');
export const reviseCardSchedule = write<
  { scheduleId: Id; dueDate: DateOnly; principalDue: Yen; source: ScheduleSource },
  Schedule
>('revise_card_schedule');
export const listCardStatements = (cardId: Id) =>
  runCommand<Statement[]>('list_card_statements', { cardId });
export const confirmCardStatement = write<StatementInput, Statement>('confirm_card_statement');
export const applyCardEvent = write<{ scheduleId: Id; eventId: Id; amount: Yen }, Schedule>('apply_card_event');
export const fundCardReserve = write<{ scheduleId: Id } & Funding, Schedule>('fund_card_reserve');
export const listSavingsGoals = () => runCommand<SavingsGoal[]>('list_savings_goals');
export const saveSavingsGoal = write<
  { goalId?: Id; name: string; target: Yen; targetDate: DateOnly | null; archive?: boolean },
  SavingsGoal
>('save_savings_goal');
export const listSavingsBuckets = () => runCommand<SavingsBucket[]>('list_savings_buckets');
export const saveSavingsBucket = write<
  { bucketId?: Id; name: string; goalId: Id | null; archive?: boolean },
  SavingsBucket
>('save_savings_bucket');
export const fundSavingsBucket = write<{ bucketId: Id } & Funding, SavingsBucket>('fund_savings_bucket');
export const getAllocationCapacity = (asOfDate: DateOnly) =>
  runCommand<AllocationCapacity[]>('get_allocation_capacity', { asOfDate });

// #10 planning
export const getFinanceSettings = () => runCommand<FinanceSettings>('get_finance_settings');
export const setFinanceSettings = write<FinanceSettings, FinanceSettings>('set_finance_settings');
export const setPaydayOverride = write<{ month: DateOnly; payday: DateOnly }, DateOnly>('set_payday_override');
export const getBudgetSummary = (asOfDate: DateOnly) =>
  runCommand<BudgetSummary>('get_budget_summary', { asOfDate });
export const setCategoryBudget = write<CategoryBudget, BudgetResult>('set_category_budget');
export const setBudgetPaymentAccount = write<
  { categoryId: Id; cycleStart: DateOnly; accountId: Id | null },
  Id | null
>('set_budget_payment_account');
export const listExpectedFlows = () => runCommand<Flow[]>('list_expected_flows');
export const createExpectedFlow = write<ExpectedFlow, Flow>('create_expected_flow');
export const overrideExpectedFlow = write<FlowOverride, FlowOverride>('override_expected_flow');
export const matchExpectedFlow = write<FlowMatch, FlowMatch>('match_expected_flow');
export const unmatchExpectedFlow = write<FlowMatch, FlowMatch>('unmatch_expected_flow');
export const getCashForecast = (from: DateOnly, to: DateOnly, scenario = false) =>
  runCommand<Forecast>('get_cash_forecast', { from, to, scenario });
export const getAvailableCash = (asOfDate: DateOnly, horizon: DateOnly) =>
  runCommand<Availability>('get_available_cash', { asOfDate, horizon });
export const getDashboard = (asOfDate: DateOnly) => runCommand<Dashboard>('get_dashboard', { asOfDate });

// #11 review
export const recordValuation = write<ValuationInput, Valuation>('record_valuation');
export const listValuations = (accountId: Id) => runCommand<Valuation[]>('list_valuations', { accountId });
export const markHoldingsChange = write<HoldingsChange, Id>('mark_holdings_change');
export const getFinancialPosition = (asOfDate: DateOnly) =>
  runCommand<Position>('get_financial_position', { asOfDate });
export const getPositionHistory = (from: DateOnly, to: DateOnly) =>
  runCommand<Position[]>('get_position_history', { from, to });
export const previewReconciliation = (input: ReconciliationInput) =>
  runCommand<Reconciliation>('preview_reconciliation', { input });
export const createReconciliation = write<ReconciliationInput, Reconciliation>('create_reconciliation');
export const getReconciliation = (reconciliationId: Id) =>
  runCommand<Reconciliation>('get_reconciliation', { reconciliationId });
export const setReconciledPosting = write<
  { reconciliationId: Id; postingId: Id; included: boolean },
  Reconciliation
>('set_reconciled_posting');
export const confirmReconciliation = write<{ reconciliationId: Id }, Reconciliation>('confirm_reconciliation');
export const reopenReconciliation = write<{ reconciliationId: Id }, Reconciliation>('reopen_reconciliation');
export const getReconciliationAudit = (reconciliationId: Id) =>
  runCommand<ReconciliationAudit[]>('get_reconciliation_audit', { reconciliationId });
export const getLedgerQuality = (asOfDate: DateOnly) =>
  runCommand<QualityIssue[]>('get_ledger_quality', { asOfDate });
