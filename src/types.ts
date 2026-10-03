export type PageKey =
  | "today"
  | "schedule"
  | "media"
  | "portfolio"
  | "growth"
  | "work"
  | "review"
  | "settings";

export interface CommandError {
  code: string;
  message: string;
  recovery?: string | null;
}

export interface SecurityStatus {
  initialized: boolean;
  unlocked: boolean;
  databasePath: string;
  keychainMode: "userPresence" | "loginKeychain" | "passphrase";
  snapshotWarning?: string | null;
  recoveryNotice?: string | null;
  sessionEpoch: number;
  lockReason?: string | null;
}

export interface AppSettings {
  theme: "light" | "dark" | "system";
  amountsHidden: boolean;
  lockMinutes: 0 | 5 | 15 | 30;
  quoteEnabled: boolean;
  quoteAutoRefresh: boolean;
  lastQuoteRefresh?: string | null;
  timezone: string;
  currency: string;
}

export interface Task {
  id: string;
  title: string;
  notes: string;
  status: "todo" | "doing" | "done";
  priority: 1 | 2 | 3;
  dueDate?: string | null;
  scheduledStart?: string | null;
  scheduledEnd?: string | null;
  recurrence: "none" | "daily" | "weekly" | "monthly";
  projectId?: string | null;
  goalId?: string | null;
  contentId?: string | null;
  completedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CalendarItem {
  id: string;
  kind: "event" | "timeblock";
  title: string;
  notes: string;
  startAt: string;
  endAt: string;
  allDay: boolean;
  recurrence: "none" | "daily" | "weekly" | "monthly";
  source: string;
  externalUid?: string | null;
  projectId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Project {
  id: string;
  name: string;
  area: string;
  status: "active" | "paused" | "completed";
  color: string;
  notes: string;
  createdAt: string;
  updatedAt: string;
}

export interface WorkLog {
  id: string;
  taskId?: string | null;
  logDate: string;
  projectId?: string | null;
  title: string;
  completed: string;
  blockers: string;
  nextSteps: string;
  minutes: number;
  energy: number;
  markdown: string;
  attachmentPath?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Goal {
  id: string;
  title: string;
  horizon: "year" | "quarter" | "month";
  status: "active" | "paused" | "completed";
  progress: number;
  notes: string;
  startDate?: string | null;
  targetDate?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Habit {
  id: string;
  name: string;
  frequency: "daily" | "weekly";
  targetPerWeek: number;
  color: string;
  active: boolean;
  checkedToday: boolean;
  streak: number;
  createdAt: string;
  updatedAt: string;
}

export interface LearningItem {
  id: string;
  title: string;
  kind: "course" | "book" | "project" | "note";
  status: "planned" | "learning" | "completed";
  progress: number;
  notes: string;
  targetDate?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ContentItem {
  id: string;
  title: string;
  platform: string;
  format: string;
  status: "idea" | "planning" | "creating" | "ready" | "published" | "review" | "archived";
  goal: string;
  tags: string[];
  publishAt?: string | null;
  notes: string;
  assetPath?: string | null;
  views: number;
  likes: number;
  comments: number;
  saves: number;
  createdAt: string;
  updatedAt: string;
}

export interface ContentMetricInput {
  contentId: string;
  recordedAt: string;
  views: number;
  likes: number;
  comments: number;
  saves: number;
  followersDelta: number;
}

export interface InvestmentAccount {
  id: string;
  name: string;
  kind: "securities" | "fund" | "cash";
  currency: string;
  createdAt: string;
  updatedAt: string;
}

export interface Instrument {
  id: string;
  code: string;
  name: string;
  kind: "stock" | "etf" | "fund" | "cash";
  market: string;
  currency: string;
  manualPrice?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface PortfolioTransaction {
  id: string;
  accountId: string;
  instrumentId: string;
  kind: "buy" | "sell" | "subscribe" | "redeem" | "dividend" | "fee" | "tax" | "adjustment";
  tradeDate: string;
  quantity: string;
  unitPrice: string;
  amount: string;
  fee: string;
  tax: string;
  notes: string;
  createdAt: string;
  updatedAt: string;
}

export interface Holding {
  accountId: string;
  instrumentId: string;
  code: string;
  name: string;
  kind: string;
  quantity: string;
  averageCost: string;
  totalCost: string;
  currentPrice: string;
  marketValue: string;
  unrealizedGain: string;
  realizedGain: string;
  priceDate?: string | null;
  priceSource: string;
}

export interface PortfolioSnapshot {
  valuationComplete?: boolean;
  missingPriceCount?: number;
  totalMarketValue: string;
  totalCost: string;
  unrealizedGain: string;
  realizedGain: string;
  holdings: Holding[];
  updatedAt?: string | null;
}

export interface ReviewSnapshot {
  id: string;
  periodType: "week" | "month";
  startDate: string;
  endDate: string;
  summary: string;
  reflection: string;
  status: "draft" | "confirmed";
  createdAt: string;
  updatedAt: string;
}

export interface Dashboard {
  portfolioError?: string | null;
  date: string;
  tasks: Task[];
  pendingTaskCount?: number;
  completedTaskCount?: number;
  nextEvent?: CalendarItem | null;
  habits: Habit[];
  workLogs: WorkLog[];
  contentItems: ContentItem[];
  portfolio: PortfolioSnapshot;
  settings: AppSettings;
}

export interface SearchResult {
  id: string;
  kind: string;
  title: string;
  subtitle: string;
}

export interface ImportReport {
  imported: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface QuoteRefreshReport {
  updated: number;
  failed: number;
  errors: string[];
  refreshedAt: string;
}

export interface BackupManifest {
  formatVersion: number;
  createdAt: string;
  appVersion: string;
  recordCount: number;
  schemaVersion?: number;
  tableRecordCounts?: Record<string, number>;
}

export interface BackupInfo {
  name: string;
  createdAt: string;
  sizeBytes: number;
  kind: string;
}
