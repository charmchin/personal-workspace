import { shanghaiInput, shanghaiToday } from "./calendar";
import type {
  AppSettings,
  CalendarItem,
  ContentItem,
  Goal,
  Habit,
  Instrument,
  InvestmentAccount,
  LearningItem,
  PortfolioTransaction,
  Project,
  ReviewSnapshot,
  Task,
  WorkLog,
} from "../types";

const today = shanghaiToday;
const nowLocal = () => shanghaiInput(new Date().toISOString());

export const defaultSettings: AppSettings = {
  theme: "system",
  amountsHidden: false,
  lockMinutes: 15,
  quoteEnabled: false,
  quoteAutoRefresh: true,
  lastQuoteRefresh: null,
  timezone: "Asia/Shanghai",
  currency: "CNY",
};

export const emptyTask = (): Task => ({
  id: "", title: "", notes: "", status: "todo", priority: 2, dueDate: today(),
  scheduledStart: null, scheduledEnd: null, recurrence: "none", projectId: null,
  goalId: null, contentId: null, completedAt: null, createdAt: "", updatedAt: "",
});

export const emptyCalendar = (): CalendarItem => ({
  id: "", kind: "event", title: "", notes: "", startAt: nowLocal(),
  endAt: shanghaiInput(new Date(Date.now() + 60 * 60 * 1000).toISOString()),
  allDay: false, recurrence: "none", source: "internal", externalUid: null,
  projectId: null, createdAt: "", updatedAt: "",
});

export const emptyProject = (): Project => ({
  id: "", name: "", area: "work", status: "active", color: "#397064", notes: "", createdAt: "", updatedAt: "",
});

export const emptyWorkLog = (): WorkLog => ({
  id: "", logDate: today(), projectId: null, taskId: null, title: "", completed: "", blockers: "",
  nextSteps: "", minutes: 0, energy: 3, markdown: "", attachmentPath: null, createdAt: "", updatedAt: "",
});

export const emptyGoal = (): Goal => ({
  id: "", title: "", horizon: "quarter", status: "active", progress: 0, notes: "",
  startDate: today(), targetDate: null, createdAt: "", updatedAt: "",
});

export const emptyHabit = (): Habit => ({
  id: "", name: "", frequency: "daily", targetPerWeek: 7, color: "#397064", active: true,
  checkedToday: false, streak: 0, createdAt: "", updatedAt: "",
});

export const emptyLearning = (): LearningItem => ({
  id: "", title: "", kind: "course", status: "planned", progress: 0, notes: "", targetDate: null,
  createdAt: "", updatedAt: "",
});

export const emptyContent = (): ContentItem => ({
  id: "", title: "", platform: "小红书", format: "图文", status: "idea", goal: "", tags: [],
  publishAt: null, notes: "", assetPath: null, views: 0, likes: 0, comments: 0, saves: 0, createdAt: "", updatedAt: "",
});

export const emptyAccount = (): InvestmentAccount => ({
  id: "", name: "", kind: "securities", currency: "CNY", createdAt: "", updatedAt: "",
});

export const emptyInstrument = (): Instrument => ({
  id: "", code: "", name: "", kind: "stock", market: "CN", currency: "CNY", manualPrice: null, createdAt: "", updatedAt: "",
});

export const emptyTransaction = (): PortfolioTransaction => ({
  id: "", accountId: "", instrumentId: "", kind: "buy", tradeDate: today(), quantity: "0",
  unitPrice: "0", amount: "0", fee: "0", tax: "0", notes: "", createdAt: "", updatedAt: "",
});

export const emptyReview = (startDate: string, endDate: string): ReviewSnapshot => ({
  id: "", periodType: "week", startDate, endDate, summary: "", reflection: "", status: "draft", createdAt: "", updatedAt: "",
});
