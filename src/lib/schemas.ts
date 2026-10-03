import { z } from "zod";

const optionalString = z.string().nullable().optional();
const commonRecord = {
  id: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
};

export const settingsSchema = z.object({
  theme: z.enum(["light", "dark", "system"]),
  amountsHidden: z.boolean(),
  lockMinutes: z.union([z.literal(0), z.literal(5), z.literal(15), z.literal(30)]),
  quoteEnabled: z.boolean(),
  quoteAutoRefresh: z.boolean(),
  lastQuoteRefresh: optionalString,
  timezone: z.string(),
  currency: z.string(),
});

export const securityStatusSchema = z.object({
  initialized: z.boolean(),
  unlocked: z.boolean(),
  databasePath: z.string(),
  keychainMode: z.enum(["userPresence", "loginKeychain", "passphrase"]),
  snapshotWarning: optionalString,
  recoveryNotice: optionalString,
  sessionEpoch: z.number().int().nonnegative(),
  lockReason: optionalString,
});

export const lockEventSchema = z.object({
  sessionEpoch: z.number().int().nonnegative(), unlocked: z.literal(false), lockReason: optionalString,
});

export const taskSchema = z.object({
  ...commonRecord,
  title: z.string(), notes: z.string(), status: z.enum(["todo", "doing", "done"]),
  priority: z.union([z.literal(1), z.literal(2), z.literal(3)]),
  dueDate: optionalString, scheduledStart: optionalString, scheduledEnd: optionalString,
  recurrence: z.enum(["none", "daily", "weekly", "monthly"]),
  projectId: optionalString, goalId: optionalString, contentId: optionalString, completedAt: optionalString,
});

export const calendarSchema = z.object({
  ...commonRecord,
  kind: z.enum(["event", "timeblock"]), title: z.string(), notes: z.string(),
  startAt: z.string(), endAt: z.string(), allDay: z.boolean(),
  recurrence: z.enum(["none", "daily", "weekly", "monthly"]), source: z.string(),
  externalUid: optionalString, projectId: optionalString,
});

export const projectSchema = z.object({
  ...commonRecord,
  name: z.string(), area: z.string(), status: z.enum(["active", "paused", "completed"]),
  color: z.string(), notes: z.string(),
});

export const workLogSchema = z.object({
  ...commonRecord,
  logDate: z.string(), projectId: optionalString, taskId: optionalString, title: z.string(), completed: z.string(),
  blockers: z.string(), nextSteps: z.string(), minutes: z.number().int().nonnegative(),
  energy: z.number().int().min(1).max(5), markdown: z.string(), attachmentPath: optionalString,
});

export const goalSchema = z.object({
  ...commonRecord,
  title: z.string(), horizon: z.enum(["year", "quarter", "month"]),
  status: z.enum(["active", "paused", "completed"]), progress: z.number().int().min(0).max(100),
  notes: z.string(), startDate: optionalString, targetDate: optionalString,
});

export const habitSchema = z.object({
  ...commonRecord,
  name: z.string(), frequency: z.enum(["daily", "weekly"]), targetPerWeek: z.number().int().min(1).max(7),
  color: z.string(), active: z.boolean(), checkedToday: z.boolean(), streak: z.number().int().nonnegative(),
});

export const learningSchema = z.object({
  ...commonRecord,
  title: z.string(), kind: z.enum(["course", "book", "project", "note"]),
  status: z.enum(["planned", "learning", "completed"]), progress: z.number().int().min(0).max(100),
  notes: z.string(), targetDate: optionalString,
});

export const contentSchema = z.object({
  ...commonRecord,
  title: z.string(), platform: z.string(), format: z.string(),
  status: z.enum(["idea", "planning", "creating", "ready", "published", "review", "archived"]),
  goal: z.string(), tags: z.array(z.string()), publishAt: optionalString, notes: z.string(), assetPath: optionalString,
  views: z.number().int().nonnegative(), likes: z.number().int().nonnegative(),
  comments: z.number().int().nonnegative(), saves: z.number().int().nonnegative(),
});

export const accountSchema = z.object({
  ...commonRecord,
  name: z.string(), kind: z.enum(["securities", "fund", "cash"]), currency: z.string(),
});

export const instrumentSchema = z.object({
  ...commonRecord,
  code: z.string(), name: z.string(), kind: z.enum(["stock", "etf", "fund", "cash"]),
  market: z.string(), currency: z.string(), manualPrice: optionalString,
});

export const transactionSchema = z.object({
  ...commonRecord,
  accountId: z.string(), instrumentId: z.string(),
  kind: z.enum(["buy", "sell", "subscribe", "redeem", "dividend", "fee", "tax", "adjustment"]),
  tradeDate: z.string(), quantity: z.string(), unitPrice: z.string(), amount: z.string(),
  fee: z.string(), tax: z.string(), notes: z.string(),
});

const requiredText = z.string().trim().min(1, "请填写必填内容").max(240, "内容不能超过 240 个字符");
const decimalInput = z.string().max(128, "金融数值过长").regex(/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/, "请使用十进制数值，不使用科学计数法");
const requestSchemas: Record<string, z.ZodType> = {
  upsert_task: z.object({ task: taskSchema.extend({ title: requiredText }) }),
  create_task_from_work_log: z.object({ logId: requiredText, task: taskSchema.extend({ title: requiredText, id: z.literal("") }) }),
  upsert_calendar_item: z.object({ item: calendarSchema.extend({ title: requiredText }) }),
  upsert_project: z.object({ project: projectSchema.extend({ name: requiredText }) }),
  upsert_work_log: z.object({ log: workLogSchema.extend({ title: requiredText }) }),
  upsert_goal: z.object({ goal: goalSchema.extend({ title: requiredText }) }),
  upsert_habit: z.object({ habit: habitSchema.extend({ name: requiredText }) }),
  upsert_learning_item: z.object({ item: learningSchema.extend({ title: requiredText }) }),
  upsert_content_item: z.object({ item: contentSchema.extend({ title: requiredText, platform: requiredText, format: requiredText }) }),
  upsert_investment_account: z.object({ account: accountSchema.extend({ name: requiredText, currency: z.literal("CNY") }) }),
  upsert_instrument: z.object({ instrument: instrumentSchema.extend({ code: requiredText, name: requiredText, manualPrice: decimalInput.nullable().optional() }) }),
  upsert_portfolio_transaction: z.object({ transaction: transactionSchema.extend({ accountId: requiredText, instrumentId: requiredText, quantity: decimalInput, unitPrice: decimalInput, amount: decimalInput, fee: decimalInput, tax: decimalInput }) }),
  update_settings: z.object({ settings: settingsSchema.extend({ timezone: z.literal("Asia/Shanghai"), currency: z.literal("CNY") }) }),
};

export function validateCommandRequest(command: string, args?: Record<string, unknown>) {
  // Backend validation remains authoritative. Unknown/read-only commands keep
  // their existing contract; never include input values in a validation error.
  const schema = requestSchemas[command];
  if (schema) schema.parse(args);
}

const holdingSchema = z.object({
  accountId: z.string(), instrumentId: z.string(), code: z.string(), name: z.string(), kind: z.string(),
  quantity: z.string(), averageCost: z.string(), totalCost: z.string(), currentPrice: z.string(),
  marketValue: z.string(), unrealizedGain: z.string(), realizedGain: z.string(),
  priceDate: optionalString, priceSource: z.string(),
});

export const portfolioSchema = z.object({
  valuationComplete: z.boolean().default(true), missingPriceCount: z.number().int().nonnegative().default(0),
  totalMarketValue: z.string(), totalCost: z.string(), unrealizedGain: z.string(), realizedGain: z.string(),
  holdings: z.array(holdingSchema), updatedAt: optionalString,
});

export const reviewSchema = z.object({
  ...commonRecord,
  periodType: z.enum(["week", "month"]), startDate: z.string(), endDate: z.string(),
  summary: z.string(), reflection: z.string(), status: z.enum(["draft", "confirmed"]),
});

const dashboardSchema = z.object({
  pendingTaskCount: z.number().int().nonnegative().optional(), completedTaskCount: z.number().int().nonnegative().optional(),
  portfolioError: optionalString,
  date: z.string(), tasks: z.array(taskSchema), nextEvent: calendarSchema.nullable().optional(),
  habits: z.array(habitSchema), workLogs: z.array(workLogSchema), contentItems: z.array(contentSchema),
  portfolio: portfolioSchema, settings: settingsSchema,
});

const importReportSchema = z.object({
  imported: z.number().int().nonnegative(), updated: z.number().int().nonnegative(),
  skipped: z.number().int().nonnegative(), errors: z.array(z.string()),
});
const quoteReportSchema = z.object({
  updated: z.number().int().nonnegative(), failed: z.number().int().nonnegative(),
  errors: z.array(z.string()), refreshedAt: z.string(),
});
const backupManifestSchema = z.object({
  formatVersion: z.union([z.literal(1), z.literal(2)]), createdAt: z.string(), appVersion: z.string(),
  recordCount: z.number().int().nonnegative(),
  schemaVersion: z.number().int().positive().optional(),
  tableRecordCounts: z.record(z.string(), z.number().int().nonnegative()).optional(),
}).refine((manifest) => manifest.formatVersion !== 2 ||
  (manifest.schemaVersion !== undefined && manifest.tableRecordCounts !== undefined),
  "新版备份缺少结构版本或逐表记录数");
const backupInfoSchema = z.object({
  name: z.string(), createdAt: z.string(), sizeBytes: z.number().int().nonnegative(),
  kind: z.enum(["daily", "weekly", "latest", "recovery"]),
});
const searchResultSchema = z.object({ id: z.string(), kind: z.string(), title: z.string(), subtitle: z.string() });

const responseSchemas: Record<string, z.ZodType> = {
  security_status: securityStatusSchema, security_unlock: securityStatusSchema,
  security_initialize_with_password: securityStatusSchema, security_unlock_with_password: securityStatusSchema,
  security_change_password: securityStatusSchema, security_lock: securityStatusSchema,
  security_activity: z.null(),
  get_dashboard: dashboardSchema,
  list_tasks: z.array(taskSchema), upsert_task: taskSchema, toggle_task: taskSchema,
  list_calendar_items: z.array(calendarSchema), upsert_calendar_item: calendarSchema,
  list_projects: z.array(projectSchema), upsert_project: projectSchema,
  list_work_logs: z.array(workLogSchema), upsert_work_log: workLogSchema, create_task_from_work_log: taskSchema,
  list_goals: z.array(goalSchema), upsert_goal: goalSchema,
  list_habits: z.array(habitSchema), upsert_habit: habitSchema, check_habit: habitSchema,
  list_learning_items: z.array(learningSchema), upsert_learning_item: learningSchema,
  list_content_items: z.array(contentSchema), upsert_content_item: contentSchema, add_content_metric: contentSchema,
  list_investment_accounts: z.array(accountSchema), upsert_investment_account: accountSchema,
  list_instruments: z.array(instrumentSchema), upsert_instrument: instrumentSchema,
  list_portfolio_transactions: z.array(transactionSchema), upsert_portfolio_transaction: transactionSchema,
  get_portfolio_snapshot: portfolioSchema,
  list_reviews: z.array(reviewSchema), upsert_review: reviewSchema,
  generate_weekly_summary: z.string(), search_records: z.array(searchResultSchema),
  get_settings: settingsSchema, update_settings: settingsSchema, has_tushare_token: z.boolean(),
  refresh_tushare_quotes: quoteReportSchema,
  import_portfolio_csv: importReportSchema, import_ics: importReportSchema, export_ics: z.number().int().nonnegative(),
  export_backup: backupManifestSchema, restore_backup: backupManifestSchema,
  list_backups: z.array(backupInfoSchema), restore_snapshot: z.number().int().nonnegative(),
};

export function validateCommandResponse(command: string, value: unknown): unknown {
  const schema = responseSchemas[command];
  return schema ? schema.parse(value) : value;
}
