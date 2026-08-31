use std::{collections::HashMap, str::FromStr};

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, Months, NaiveDate, NaiveDateTime, TimeZone,
    Utc,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    database::{load_settings, now_utc, save_settings},
    error::{CommandError, CommandResult},
    models::*,
};

fn identifier(value: &str) -> String {
    if value.trim().is_empty() {
        Uuid::now_v7().to_string()
    } else {
        value.to_owned()
    }
}

fn require_text(value: &str, field: &str) -> CommandResult<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{field}不能为空"),
        ))
    } else if value.chars().count() > 240 {
        Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{field}不能超过 240 个字符"),
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn require_choice(value: &str, field: &str, choices: &[&str]) -> CommandResult<()> {
    if choices.contains(&value) {
        Ok(())
    } else {
        Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{field}无效"),
        ))
    }
}

fn validate_text_length(value: &str, field: &str, max_bytes: usize) -> CommandResult<()> {
    if value.len() > max_bytes {
        Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{field}超过可保存的长度限制"),
        ))
    } else {
        Ok(())
    }
}

fn validate_optional_text_length(
    value: Option<&str>,
    field: &str,
    max_bytes: usize,
) -> CommandResult<()> {
    if let Some(value) = value {
        validate_text_length(value, field, max_bytes)?;
    }
    Ok(())
}

fn parse_date(value: &str, field: &str) -> CommandResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| CommandError::new("VALIDATION_ERROR", format!("{field}格式无效")))
}

fn validate_optional_date(value: Option<&str>, field: &str) -> CommandResult<()> {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        parse_date(value, field)?;
    }
    Ok(())
}

fn parse_datetime(value: &str, field: &str) -> CommandResult<NaiveDateTime> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(value.naive_utc());
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
        .map_err(|_| CommandError::new("VALIDATION_ERROR", format!("{field}格式无效")))?;
    FixedOffset::east_opt(8 * 60 * 60)
        .and_then(|offset| offset.from_local_datetime(&naive).single())
        .map(|value| value.naive_utc())
        .ok_or_else(|| CommandError::new("VALIDATION_ERROR", format!("{field}无效")))
}

fn validate_optional_datetime(value: Option<&str>, field: &str) -> CommandResult<()> {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        parse_datetime(value, field)?;
    }
    Ok(())
}

fn parse_decimal(value: &str, field: &str) -> CommandResult<Decimal> {
    Decimal::from_str(value.trim())
        .map_err(|_| CommandError::new("INVALID_DECIMAL", format!("{field}必须是有效的十进制数字")))
}

fn decimal_string(value: Decimal) -> String {
    value.normalize().to_string()
}

fn map_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        due_date: row.get(5)?,
        scheduled_start: row.get(6)?,
        scheduled_end: row.get(7)?,
        recurrence: row.get(8)?,
        project_id: row.get(9)?,
        goal_id: row.get(10)?,
        content_id: row.get(11)?,
        completed_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

const TASK_FIELDS: &str = "id, title, notes, status, priority, due_date, scheduled_start, scheduled_end, recurrence, project_id, goal_id, content_id, completed_at, created_at, updated_at";

pub fn list_tasks(connection: &Connection) -> CommandResult<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_FIELDS} FROM tasks ORDER BY CASE status WHEN 'todo' THEN 0 WHEN 'doing' THEN 1 ELSE 2 END, priority ASC, COALESCE(due_date, '9999-12-31'), created_at DESC"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], map_task)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_task(connection: &Connection, id: &str) -> CommandResult<Task> {
    let sql = format!("SELECT {TASK_FIELDS} FROM tasks WHERE id = ?1");
    connection
        .query_row(&sql, [id], map_task)
        .map_err(Into::into)
}

pub fn upsert_task(connection: &Connection, mut task: Task) -> CommandResult<Task> {
    task.id = identifier(&task.id);
    task.title = require_text(&task.title, "任务标题")?;
    if !(1..=3).contains(&task.priority) {
        return Err(CommandError::new("VALIDATION_ERROR", "任务优先级无效"));
    }
    if !matches!(task.status.as_str(), "todo" | "doing" | "done") {
        return Err(CommandError::new("VALIDATION_ERROR", "任务状态无效"));
    }
    if !matches!(
        task.recurrence.as_str(),
        "none" | "daily" | "weekly" | "monthly"
    ) {
        return Err(CommandError::new("VALIDATION_ERROR", "重复规则无效"));
    }
    validate_optional_date(task.due_date.as_deref(), "截止日期")?;
    validate_optional_datetime(task.scheduled_start.as_deref(), "计划开始时间")?;
    validate_optional_datetime(task.scheduled_end.as_deref(), "计划结束时间")?;
    if let (Some(start), Some(end)) = (&task.scheduled_start, &task.scheduled_end)
        && parse_datetime(end, "计划结束时间")? < parse_datetime(start, "计划开始时间")?
    {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "计划结束时间不能早于开始时间",
        ));
    }
    validate_text_length(&task.notes, "任务说明", 100_000)?;
    let now = now_utc();
    let created_at = if task.created_at.is_empty() {
        now.clone()
    } else {
        task.created_at.clone()
    };
    let completed_at = if task.status == "done" {
        task.completed_at.clone().or_else(|| Some(now.clone()))
    } else {
        None
    };
    connection.execute(
        r#"INSERT INTO tasks(id, title, notes, status, priority, due_date, scheduled_start, scheduled_end, recurrence, project_id, goal_id, content_id, completed_at, created_at, updated_at)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
           ON CONFLICT(id) DO UPDATE SET title=excluded.title, notes=excluded.notes, status=excluded.status, priority=excluded.priority, due_date=excluded.due_date, scheduled_start=excluded.scheduled_start, scheduled_end=excluded.scheduled_end, recurrence=excluded.recurrence, project_id=excluded.project_id, goal_id=excluded.goal_id, content_id=excluded.content_id, completed_at=excluded.completed_at, updated_at=excluded.updated_at"#,
        params![task.id, task.title, task.notes, task.status, task.priority, task.due_date, task.scheduled_start, task.scheduled_end, task.recurrence, task.project_id, task.goal_id, task.content_id, completed_at, created_at, now],
    )?;
    get_task(connection, &task.id)
}

pub fn toggle_task(connection: &Connection, id: &str, completed: bool) -> CommandResult<Task> {
    let task = get_task(connection, id)?;
    let now = now_utc();
    if completed && task.recurrence != "none" {
        let base_date = task
            .due_date
            .as_deref()
            .or(task.scheduled_start.as_deref())
            .and_then(|value| value.get(..10))
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        if let Some((base_date, next_date)) = base_date.and_then(|base| {
            next_task_recurrence_date(base, &task.recurrence).map(|next| (base, next))
        }) {
            let transaction = connection.unchecked_transaction()?;
            let mut completed_occurrence = task.clone();
            completed_occurrence.id.clear();
            completed_occurrence.status = "done".into();
            completed_occurrence.recurrence = "none".into();
            completed_occurrence.completed_at = Some(now);
            completed_occurrence.created_at.clear();
            completed_occurrence.updated_at.clear();
            upsert_task(&transaction, completed_occurrence)?;

            let days = (next_date - base_date).num_days();
            let mut next_occurrence = task;
            next_occurrence.status = "todo".into();
            next_occurrence.completed_at = None;
            next_occurrence.due_date = next_occurrence
                .due_date
                .as_deref()
                .and_then(|value| shift_date_prefix(value, days));
            next_occurrence.scheduled_start = next_occurrence
                .scheduled_start
                .as_deref()
                .and_then(|value| shift_date_prefix(value, days));
            next_occurrence.scheduled_end = next_occurrence
                .scheduled_end
                .as_deref()
                .and_then(|value| shift_date_prefix(value, days));
            let result = upsert_task(&transaction, next_occurrence)?;
            transaction.commit()?;
            return Ok(result);
        }
    }
    connection.execute(
        "UPDATE tasks SET status = ?2, completed_at = ?3, updated_at = ?4 WHERE id = ?1",
        params![
            id,
            if completed { "done" } else { "todo" },
            if completed { Some(now.clone()) } else { None },
            now
        ],
    )?;
    get_task(connection, id)
}

fn next_task_recurrence_date(date: NaiveDate, recurrence: &str) -> Option<NaiveDate> {
    match recurrence {
        "daily" => date.checked_add_signed(Duration::days(1)),
        "weekly" => date.checked_add_signed(Duration::weeks(1)),
        "monthly" => (1..=12).find_map(|months| date.checked_add_months(Months::new(months))),
        _ => None,
    }
}

fn shift_date_prefix(value: &str, days: i64) -> Option<String> {
    let date = NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()?;
    let shifted = date.checked_add_signed(Duration::days(days))?;
    Some(format!(
        "{}{}",
        shifted.format("%Y-%m-%d"),
        value.get(10..).unwrap_or("")
    ))
}

fn map_calendar(row: &Row<'_>) -> rusqlite::Result<CalendarItem> {
    Ok(CalendarItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        notes: row.get(3)?,
        start_at: row.get(4)?,
        end_at: row.get(5)?,
        all_day: row.get::<_, i64>(6)? != 0,
        recurrence: row.get(7)?,
        source: row.get(8)?,
        external_uid: row.get(9)?,
        project_id: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

const CALENDAR_FIELDS: &str = "id, kind, title, notes, start_at, end_at, all_day, recurrence, source, external_uid, project_id, created_at, updated_at";

pub fn list_calendar(
    connection: &Connection,
    start: Option<&str>,
    end: Option<&str>,
) -> CommandResult<Vec<CalendarItem>> {
    let sql = format!(
        "SELECT {CALENDAR_FIELDS} FROM calendar_items WHERE (?1 IS NULL OR end_at >= ?1) AND (?2 IS NULL OR start_at <= ?2) ORDER BY start_at"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![start, end], map_calendar)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_calendar_item(connection: &Connection, id: &str) -> CommandResult<CalendarItem> {
    let sql = format!("SELECT {CALENDAR_FIELDS} FROM calendar_items WHERE id = ?1");
    connection
        .query_row(&sql, [id], map_calendar)
        .map_err(Into::into)
}

pub fn upsert_calendar(
    connection: &Connection,
    mut item: CalendarItem,
) -> CommandResult<CalendarItem> {
    item.id = identifier(&item.id);
    item.title = require_text(&item.title, "日程标题")?;
    let start = parse_datetime(&item.start_at, "日程开始时间")?;
    let end = parse_datetime(&item.end_at, "日程结束时间")?;
    if end < start {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "日程结束时间不能早于开始时间",
        ));
    }
    if !matches!(item.kind.as_str(), "event" | "timeblock") {
        return Err(CommandError::new("VALIDATION_ERROR", "日程类型无效"));
    }
    if !matches!(
        item.recurrence.as_str(),
        "none" | "daily" | "weekly" | "monthly"
    ) {
        return Err(CommandError::new("VALIDATION_ERROR", "重复规则无效"));
    }
    validate_text_length(&item.notes, "日程说明", 100_000)?;
    validate_text_length(&item.source, "日程来源", 4_096)?;
    validate_optional_text_length(item.external_uid.as_deref(), "ICS UID", 1_024)?;
    let now = now_utc();
    let created_at = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(
        r#"INSERT INTO calendar_items(id, kind, title, notes, start_at, end_at, all_day, recurrence, source, external_uid, project_id, created_at, updated_at)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
           ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, title=excluded.title, notes=excluded.notes, start_at=excluded.start_at, end_at=excluded.end_at, all_day=excluded.all_day, recurrence=excluded.recurrence, project_id=excluded.project_id, updated_at=excluded.updated_at"#,
        params![item.id, item.kind, item.title, item.notes, item.start_at, item.end_at, item.all_day as i64, item.recurrence, item.source, item.external_uid, item.project_id, created_at, now],
    )?;
    get_calendar_item(connection, &item.id)
}

fn map_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        area: row.get(2)?,
        status: row.get(3)?,
        color: row.get(4)?,
        notes: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn list_projects(connection: &Connection) -> CommandResult<Vec<Project>> {
    let mut statement = connection.prepare("SELECT id, name, area, status, color, notes, created_at, updated_at FROM projects ORDER BY CASE status WHEN 'active' THEN 0 ELSE 1 END, updated_at DESC")?;
    let rows = statement.query_map([], map_project)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn upsert_project(connection: &Connection, mut project: Project) -> CommandResult<Project> {
    project.id = identifier(&project.id);
    project.name = require_text(&project.name, "项目名称")?;
    require_choice(
        &project.area,
        "项目领域",
        &["work", "media", "growth", "personal"],
    )?;
    validate_text_length(&project.notes, "项目说明", 100_000)?;
    validate_text_length(&project.color, "项目颜色", 32)?;
    require_choice(
        &project.status,
        "项目状态",
        &["active", "paused", "completed"],
    )?;
    let now = now_utc();
    let created_at = if project.created_at.is_empty() {
        now.clone()
    } else {
        project.created_at.clone()
    };
    connection.execute(
        r#"INSERT INTO projects(id,name,area,status,color,notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
           ON CONFLICT(id) DO UPDATE SET name=excluded.name, area=excluded.area, status=excluded.status, color=excluded.color, notes=excluded.notes, updated_at=excluded.updated_at"#,
        params![project.id, project.name, project.area, project.status, project.color, project.notes, created_at, now],
    )?;
    connection.query_row("SELECT id, name, area, status, color, notes, created_at, updated_at FROM projects WHERE id=?1", [&project.id], map_project).map_err(Into::into)
}

fn map_work_log(row: &Row<'_>) -> rusqlite::Result<WorkLog> {
    Ok(WorkLog {
        id: row.get(0)?,
        log_date: row.get(1)?,
        project_id: row.get(2)?,
        title: row.get(3)?,
        completed: row.get(4)?,
        blockers: row.get(5)?,
        next_steps: row.get(6)?,
        minutes: row.get(7)?,
        energy: row.get(8)?,
        markdown: row.get(9)?,
        attachment_path: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub fn list_work_logs(connection: &Connection) -> CommandResult<Vec<WorkLog>> {
    let mut statement = connection.prepare("SELECT id, log_date, project_id, title, completed, blockers, next_steps, minutes, energy, markdown, attachment_path, created_at, updated_at FROM work_logs ORDER BY log_date DESC, created_at DESC")?;
    let rows = statement.query_map([], map_work_log)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn upsert_work_log(connection: &Connection, mut log: WorkLog) -> CommandResult<WorkLog> {
    log.id = identifier(&log.id);
    log.title = require_text(&log.title, "日志标题")?;
    if !(1..=5).contains(&log.energy) || !(0..=10_080).contains(&log.minutes) {
        return Err(CommandError::new("VALIDATION_ERROR", "精力或耗时数值无效"));
    }
    parse_date(&log.log_date, "日志日期")?;
    validate_text_length(&log.completed, "完成事项", 100_000)?;
    validate_text_length(&log.blockers, "问题记录", 100_000)?;
    validate_text_length(&log.next_steps, "下一步", 100_000)?;
    validate_text_length(&log.markdown, "Markdown 日志", 1_000_000)?;
    validate_optional_text_length(log.attachment_path.as_deref(), "附件路径", 4_096)?;
    let now = now_utc();
    let created_at = if log.created_at.is_empty() {
        now.clone()
    } else {
        log.created_at.clone()
    };
    connection.execute(
        r#"INSERT INTO work_logs(id,log_date,project_id,title,completed,blockers,next_steps,minutes,energy,markdown,attachment_path,created_at,updated_at)
           VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
           ON CONFLICT(id) DO UPDATE SET log_date=excluded.log_date, project_id=excluded.project_id, title=excluded.title, completed=excluded.completed, blockers=excluded.blockers, next_steps=excluded.next_steps, minutes=excluded.minutes, energy=excluded.energy, markdown=excluded.markdown, attachment_path=excluded.attachment_path, updated_at=excluded.updated_at"#,
        params![log.id, log.log_date, log.project_id, log.title, log.completed, log.blockers, log.next_steps, log.minutes, log.energy, log.markdown, log.attachment_path, created_at, now],
    )?;
    connection.query_row("SELECT id, log_date, project_id, title, completed, blockers, next_steps, minutes, energy, markdown, attachment_path, created_at, updated_at FROM work_logs WHERE id=?1", [&log.id], map_work_log).map_err(Into::into)
}

fn map_goal(row: &Row<'_>) -> rusqlite::Result<Goal> {
    Ok(Goal {
        id: row.get(0)?,
        title: row.get(1)?,
        horizon: row.get(2)?,
        status: row.get(3)?,
        progress: row.get(4)?,
        notes: row.get(5)?,
        start_date: row.get(6)?,
        target_date: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn list_goals(connection: &Connection) -> CommandResult<Vec<Goal>> {
    let mut statement = connection.prepare("SELECT id,title,horizon,status,progress,notes,start_date,target_date,created_at,updated_at FROM goals ORDER BY CASE status WHEN 'active' THEN 0 ELSE 1 END, target_date")?;
    let rows = statement.query_map([], map_goal)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn upsert_goal(connection: &Connection, mut goal: Goal) -> CommandResult<Goal> {
    goal.id = identifier(&goal.id);
    goal.title = require_text(&goal.title, "目标标题")?;
    if !(0..=100).contains(&goal.progress) {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "目标进度必须在 0 到 100 之间",
        ));
    }
    require_choice(&goal.horizon, "目标周期", &["year", "quarter", "month"])?;
    require_choice(&goal.status, "目标状态", &["active", "paused", "completed"])?;
    validate_optional_date(goal.start_date.as_deref(), "目标开始日期")?;
    validate_optional_date(goal.target_date.as_deref(), "目标日期")?;
    if let (Some(start), Some(target)) = (&goal.start_date, &goal.target_date)
        && parse_date(target, "目标日期")? < parse_date(start, "目标开始日期")?
    {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "目标日期不能早于开始日期",
        ));
    }
    validate_text_length(&goal.notes, "目标说明", 100_000)?;
    let now = now_utc();
    let created_at = if goal.created_at.is_empty() {
        now.clone()
    } else {
        goal.created_at.clone()
    };
    connection.execute(
        r#"INSERT INTO goals(id,title,horizon,status,progress,notes,start_date,target_date,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
           ON CONFLICT(id) DO UPDATE SET title=excluded.title,horizon=excluded.horizon,status=excluded.status,progress=excluded.progress,notes=excluded.notes,start_date=excluded.start_date,target_date=excluded.target_date,updated_at=excluded.updated_at"#,
        params![goal.id,goal.title,goal.horizon,goal.status,goal.progress,goal.notes,goal.start_date,goal.target_date,created_at,now],
    )?;
    connection.query_row("SELECT id,title,horizon,status,progress,notes,start_date,target_date,created_at,updated_at FROM goals WHERE id=?1", [&goal.id], map_goal).map_err(Into::into)
}

fn calculate_streak(
    connection: &Connection,
    habit_id: &str,
    today: NaiveDate,
) -> rusqlite::Result<i64> {
    let mut statement = connection.prepare("SELECT check_date FROM habit_checks WHERE habit_id=?1 AND value > 0 ORDER BY check_date DESC LIMIT 366")?;
    let dates = statement
        .query_map([habit_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected = today;
    let mut streak = 0;
    for value in dates {
        if let Ok(date) = NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
            if date == expected {
                streak += 1;
                expected -= Duration::days(1);
            } else if date < expected {
                break;
            }
        }
    }
    Ok(streak)
}

pub fn list_habits(connection: &Connection, date: &str) -> CommandResult<Vec<Habit>> {
    let today = parse_date(date, "习惯日期")?;
    let mut statement = connection.prepare(
        "SELECT h.id,h.name,h.frequency,h.target_per_week,h.color,h.active,h.created_at,h.updated_at, EXISTS(SELECT 1 FROM habit_checks c WHERE c.habit_id=h.id AND c.check_date=?1 AND c.value > 0) FROM habits h ORDER BY h.active DESC,h.created_at"
    )?;
    let mapped = statement.query_map([date], |row| {
        Ok(Habit {
            id: row.get(0)?,
            name: row.get(1)?,
            frequency: row.get(2)?,
            target_per_week: row.get(3)?,
            color: row.get(4)?,
            active: row.get::<_, i64>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            checked_today: row.get::<_, i64>(8)? != 0,
            streak: 0,
        })
    })?;
    let mut habits = mapped.collect::<Result<Vec<_>, _>>()?;
    for habit in &mut habits {
        habit.streak = calculate_streak(connection, &habit.id, today)?;
    }
    Ok(habits)
}

pub fn upsert_habit(connection: &Connection, mut habit: Habit) -> CommandResult<Habit> {
    habit.id = identifier(&habit.id);
    habit.name = require_text(&habit.name, "习惯名称")?;
    if !(1..=7).contains(&habit.target_per_week) {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "每周目标次数必须在 1 到 7 之间",
        ));
    }
    require_choice(&habit.frequency, "习惯频率", &["daily", "weekly"])?;
    validate_text_length(&habit.color, "习惯颜色", 32)?;
    let now = now_utc();
    let created_at = if habit.created_at.is_empty() {
        now.clone()
    } else {
        habit.created_at.clone()
    };
    connection.execute(
        r#"INSERT INTO habits(id,name,frequency,target_per_week,color,active,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
           ON CONFLICT(id) DO UPDATE SET name=excluded.name,frequency=excluded.frequency,target_per_week=excluded.target_per_week,color=excluded.color,active=excluded.active,updated_at=excluded.updated_at"#,
        params![habit.id,habit.name,habit.frequency,habit.target_per_week,habit.color,habit.active as i64,created_at,now],
    )?;
    list_habits(connection, &Local::now().format("%Y-%m-%d").to_string())?
        .into_iter()
        .find(|item| item.id == habit.id)
        .ok_or_else(|| CommandError::new("NOT_FOUND", "未找到已保存的习惯"))
}

pub fn check_habit(
    connection: &Connection,
    habit_id: &str,
    date: &str,
    checked: bool,
) -> CommandResult<Habit> {
    parse_date(date, "打卡日期")?;
    if checked {
        connection.execute(
            "INSERT INTO habit_checks(id,habit_id,check_date,value,note) VALUES(?1,?2,?3,1,'') ON CONFLICT(habit_id,check_date) DO UPDATE SET value=1",
            params![Uuid::now_v7().to_string(),habit_id,date],
        )?;
    } else {
        connection.execute(
            "DELETE FROM habit_checks WHERE habit_id=?1 AND check_date=?2",
            params![habit_id, date],
        )?;
    }
    list_habits(connection, date)?
        .into_iter()
        .find(|item| item.id == habit_id)
        .ok_or_else(|| CommandError::new("NOT_FOUND", "未找到习惯"))
}

fn map_learning(row: &Row<'_>) -> rusqlite::Result<LearningItem> {
    Ok(LearningItem {
        id: row.get(0)?,
        title: row.get(1)?,
        kind: row.get(2)?,
        status: row.get(3)?,
        progress: row.get(4)?,
        notes: row.get(5)?,
        target_date: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub fn list_learning(connection: &Connection) -> CommandResult<Vec<LearningItem>> {
    let mut statement = connection.prepare("SELECT id,title,kind,status,progress,notes,target_date,created_at,updated_at FROM learning_items ORDER BY CASE status WHEN 'learning' THEN 0 WHEN 'planned' THEN 1 ELSE 2 END,target_date")?;
    let rows = statement.query_map([], map_learning)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn upsert_learning(
    connection: &Connection,
    mut item: LearningItem,
) -> CommandResult<LearningItem> {
    item.id = identifier(&item.id);
    item.title = require_text(&item.title, "学习项目标题")?;
    if !(0..=100).contains(&item.progress) {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "学习进度必须在 0 到 100 之间",
        ));
    }
    require_choice(
        &item.kind,
        "学习类型",
        &["course", "book", "project", "note"],
    )?;
    require_choice(
        &item.status,
        "学习状态",
        &["planned", "learning", "completed"],
    )?;
    validate_optional_date(item.target_date.as_deref(), "学习目标日期")?;
    validate_text_length(&item.notes, "学习笔记", 1_000_000)?;
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(r#"INSERT INTO learning_items(id,title,kind,status,progress,notes,target_date,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
      ON CONFLICT(id) DO UPDATE SET title=excluded.title,kind=excluded.kind,status=excluded.status,progress=excluded.progress,notes=excluded.notes,target_date=excluded.target_date,updated_at=excluded.updated_at"#,
      params![item.id,item.title,item.kind,item.status,item.progress,item.notes,item.target_date,created,now])?;
    connection.query_row("SELECT id,title,kind,status,progress,notes,target_date,created_at,updated_at FROM learning_items WHERE id=?1", [&item.id], map_learning).map_err(Into::into)
}

fn map_content(row: &Row<'_>) -> rusqlite::Result<ContentItem> {
    let tags_json: String = row.get(6)?;
    Ok(ContentItem {
        id: row.get(0)?,
        title: row.get(1)?,
        platform: row.get(2)?,
        format: row.get(3)?,
        status: row.get(4)?,
        goal: row.get(5)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        publish_at: row.get(7)?,
        notes: row.get(8)?,
        asset_path: row.get(9)?,
        views: row.get(10)?,
        likes: row.get(11)?,
        comments: row.get(12)?,
        saves: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

const CONTENT_SELECT: &str = r#"SELECT c.id,c.title,c.platform,c.format,c.status,c.goal,c.tags_json,c.publish_at,c.notes,c.asset_path,
 COALESCE((SELECT m.views FROM content_metrics m WHERE m.content_id=c.id ORDER BY m.recorded_at DESC LIMIT 1),0),
 COALESCE((SELECT m.likes FROM content_metrics m WHERE m.content_id=c.id ORDER BY m.recorded_at DESC LIMIT 1),0),
 COALESCE((SELECT m.comments FROM content_metrics m WHERE m.content_id=c.id ORDER BY m.recorded_at DESC LIMIT 1),0),
 COALESCE((SELECT m.saves FROM content_metrics m WHERE m.content_id=c.id ORDER BY m.recorded_at DESC LIMIT 1),0),
 c.created_at,c.updated_at FROM content_items c"#;

pub fn list_content(connection: &Connection) -> CommandResult<Vec<ContentItem>> {
    let mut statement = connection.prepare(&format!("{CONTENT_SELECT} ORDER BY CASE c.status WHEN 'idea' THEN 0 WHEN 'planning' THEN 1 WHEN 'creating' THEN 2 WHEN 'ready' THEN 3 WHEN 'published' THEN 4 WHEN 'review' THEN 5 ELSE 6 END,c.publish_at,c.updated_at DESC"))?;
    let rows = statement.query_map([], map_content)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn upsert_content(
    connection: &Connection,
    mut item: ContentItem,
) -> CommandResult<ContentItem> {
    item.id = identifier(&item.id);
    item.title = require_text(&item.title, "内容标题")?;
    if !matches!(
        item.status.as_str(),
        "idea" | "planning" | "creating" | "ready" | "published" | "review" | "archived"
    ) {
        return Err(CommandError::new("VALIDATION_ERROR", "内容状态无效"));
    }
    validate_optional_datetime(item.publish_at.as_deref(), "发布时间")?;
    item.platform = require_text(&item.platform, "发布平台")?;
    item.format = require_text(&item.format, "内容形式")?;
    validate_text_length(&item.goal, "内容目标", 100_000)?;
    validate_text_length(&item.notes, "内容笔记", 1_000_000)?;
    validate_optional_text_length(item.asset_path.as_deref(), "素材路径", 4_096)?;
    item.tags = item
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect();
    for tag in &item.tags {
        validate_text_length(tag, "内容标签", 128)?;
    }
    item.tags.dedup();
    item.tags.truncate(12);
    let tags = serde_json::to_string(&item.tags)?;
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(r#"INSERT INTO content_items(id,title,platform,format,status,goal,tags_json,publish_at,notes,asset_path,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
      ON CONFLICT(id) DO UPDATE SET title=excluded.title,platform=excluded.platform,format=excluded.format,status=excluded.status,goal=excluded.goal,tags_json=excluded.tags_json,publish_at=excluded.publish_at,notes=excluded.notes,asset_path=excluded.asset_path,updated_at=excluded.updated_at"#,
      params![item.id,item.title,item.platform,item.format,item.status,item.goal,tags,item.publish_at,item.notes,item.asset_path,created,now])?;
    connection
        .query_row(
            &format!("{CONTENT_SELECT} WHERE c.id=?1"),
            [&item.id],
            map_content,
        )
        .map_err(Into::into)
}

pub fn add_content_metric(
    connection: &Connection,
    input: ContentMetricInput,
) -> CommandResult<ContentItem> {
    parse_datetime(&input.recorded_at, "指标记录时间")?;
    for (name, value) in [
        ("浏览量", input.views),
        ("点赞", input.likes),
        ("评论", input.comments),
        ("收藏", input.saves),
    ] {
        if value < 0 {
            return Err(CommandError::new(
                "VALIDATION_ERROR",
                format!("{name}不能为负数"),
            ));
        }
    }
    connection.execute(r#"INSERT INTO content_metrics(id,content_id,recorded_at,views,likes,comments,saves,followers_delta) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
      ON CONFLICT(content_id,recorded_at) DO UPDATE SET views=excluded.views,likes=excluded.likes,comments=excluded.comments,saves=excluded.saves,followers_delta=excluded.followers_delta"#,
      params![Uuid::now_v7().to_string(),input.content_id,input.recorded_at,input.views,input.likes,input.comments,input.saves,input.followers_delta])?;
    connection
        .query_row(
            &format!("{CONTENT_SELECT} WHERE c.id=?1"),
            [&input.content_id],
            map_content,
        )
        .map_err(Into::into)
}

fn map_account(row: &Row<'_>) -> rusqlite::Result<InvestmentAccount> {
    Ok(InvestmentAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        currency: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}
pub fn list_accounts(connection: &Connection) -> CommandResult<Vec<InvestmentAccount>> {
    let mut s=connection.prepare("SELECT id,name,kind,currency,created_at,updated_at FROM investment_accounts ORDER BY created_at")?;
    let r = s.query_map([], map_account)?;
    Ok(r.collect::<Result<Vec<_>, _>>()?)
}
pub fn upsert_account(
    connection: &Connection,
    mut item: InvestmentAccount,
) -> CommandResult<InvestmentAccount> {
    item.id = identifier(&item.id);
    item.name = require_text(&item.name, "账户名称")?;
    require_choice(&item.kind, "账户类型", &["securities", "fund", "cash"])?;
    require_choice(&item.currency, "账户货币", &["CNY"])?;
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(r#"INSERT INTO investment_accounts(id,name,kind,currency,created_at,updated_at)VALUES(?1,?2,?3,?4,?5,?6)ON CONFLICT(id)DO UPDATE SET name=excluded.name,kind=excluded.kind,currency=excluded.currency,updated_at=excluded.updated_at"#,params![item.id,item.name,item.kind,item.currency,created,now])?;
    connection.query_row("SELECT id,name,kind,currency,created_at,updated_at FROM investment_accounts WHERE id=?1",[&item.id],map_account).map_err(Into::into)
}

fn map_instrument(row: &Row<'_>) -> rusqlite::Result<Instrument> {
    Ok(Instrument {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        market: row.get(4)?,
        currency: row.get(5)?,
        manual_price: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}
pub fn list_instruments(connection: &Connection) -> CommandResult<Vec<Instrument>> {
    let mut s=connection.prepare("SELECT id,code,name,kind,market,currency,manual_price,created_at,updated_at FROM instruments ORDER BY code")?;
    let r = s.query_map([], map_instrument)?;
    Ok(r.collect::<Result<Vec<_>, _>>()?)
}
pub fn upsert_instrument(
    connection: &Connection,
    mut item: Instrument,
) -> CommandResult<Instrument> {
    item.id = identifier(&item.id);
    item.code = require_text(&item.code, "证券代码")?.to_uppercase();
    item.name = require_text(&item.name, "证券名称")?;
    require_choice(&item.kind, "资产类型", &["stock", "etf", "fund", "cash"])?;
    require_choice(&item.market, "资产市场", &["CN"])?;
    require_choice(&item.currency, "资产货币", &["CNY"])?;
    if let Some(price) = &item.manual_price {
        let parsed = parse_decimal(price, "手工价格")?;
        if parsed < Decimal::ZERO {
            return Err(CommandError::new("VALIDATION_ERROR", "手工价格不能为负数"));
        }
    }
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(r#"INSERT INTO instruments(id,code,name,kind,market,currency,manual_price,created_at,updated_at)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)ON CONFLICT(id)DO UPDATE SET code=excluded.code,name=excluded.name,kind=excluded.kind,market=excluded.market,currency=excluded.currency,manual_price=excluded.manual_price,updated_at=excluded.updated_at"#,params![item.id,item.code,item.name,item.kind,item.market,item.currency,item.manual_price,created,now])?;
    connection.query_row("SELECT id,code,name,kind,market,currency,manual_price,created_at,updated_at FROM instruments WHERE id=?1",[&item.id],map_instrument).map_err(Into::into)
}

fn map_transaction(row: &Row<'_>) -> rusqlite::Result<PortfolioTransaction> {
    Ok(PortfolioTransaction {
        id: row.get(0)?,
        account_id: row.get(1)?,
        instrument_id: row.get(2)?,
        kind: row.get(3)?,
        trade_date: row.get(4)?,
        quantity: row.get(5)?,
        unit_price: row.get(6)?,
        amount: row.get(7)?,
        fee: row.get(8)?,
        tax: row.get(9)?,
        notes: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}
pub fn list_transactions(connection: &Connection) -> CommandResult<Vec<PortfolioTransaction>> {
    let mut s=connection.prepare("SELECT id,account_id,instrument_id,kind,trade_date,quantity,unit_price,amount,fee,tax,notes,created_at,updated_at FROM portfolio_transactions ORDER BY trade_date DESC,created_at DESC")?;
    let r = s.query_map([], map_transaction)?;
    Ok(r.collect::<Result<Vec<_>, _>>()?)
}
pub fn upsert_transaction(
    connection: &Connection,
    mut item: PortfolioTransaction,
) -> CommandResult<PortfolioTransaction> {
    item.id = identifier(&item.id);
    if !matches!(
        item.kind.as_str(),
        "buy" | "sell" | "subscribe" | "redeem" | "dividend" | "fee" | "tax" | "adjustment"
    ) {
        return Err(CommandError::new("VALIDATION_ERROR", "交易类型无效"));
    }
    parse_date(&item.trade_date, "交易日期")?;
    if item.account_id.trim().is_empty() || item.instrument_id.trim().is_empty() {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "交易必须关联账户和资产",
        ));
    }
    for (name, value) in [
        ("数量", &item.quantity),
        ("单价", &item.unit_price),
        ("金额", &item.amount),
        ("手续费", &item.fee),
        ("税费", &item.tax),
    ] {
        let parsed = parse_decimal(value, name)?;
        if parsed < Decimal::ZERO {
            return Err(CommandError::new(
                "VALIDATION_ERROR",
                format!("{name}不能为负数"),
            ));
        }
    }
    validate_text_length(&item.notes, "交易备注", 100_000)?;
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let saved = persist_transaction(&transaction, &item, &created, &now)?;
        transaction.commit()?;
        Ok(saved)
    } else {
        persist_transaction(connection, &item, &created, &now)
    }
}

fn persist_transaction(
    connection: &Connection,
    item: &PortfolioTransaction,
    created: &str,
    now: &str,
) -> CommandResult<PortfolioTransaction> {
    connection.execute(r#"INSERT INTO portfolio_transactions(id,account_id,instrument_id,kind,trade_date,quantity,unit_price,amount,fee,tax,notes,created_at,updated_at)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)ON CONFLICT(id)DO UPDATE SET account_id=excluded.account_id,instrument_id=excluded.instrument_id,kind=excluded.kind,trade_date=excluded.trade_date,quantity=excluded.quantity,unit_price=excluded.unit_price,amount=excluded.amount,fee=excluded.fee,tax=excluded.tax,notes=excluded.notes,updated_at=excluded.updated_at"#,params![&item.id,&item.account_id,&item.instrument_id,&item.kind,&item.trade_date,&item.quantity,&item.unit_price,&item.amount,&item.fee,&item.tax,&item.notes,created,now])?;
    let saved = connection.query_row("SELECT id,account_id,instrument_id,kind,trade_date,quantity,unit_price,amount,fee,tax,notes,created_at,updated_at FROM portfolio_transactions WHERE id=?1",[&item.id],map_transaction)?;
    portfolio_snapshot(connection)?;
    Ok(saved)
}

pub fn upsert_price(connection: &Connection, input: PricePointInput) -> CommandResult<()> {
    parse_date(&input.price_date, "价格日期")?;
    require_choice(&input.source, "价格来源", &["manual", "tushare"])?;
    let price = parse_decimal(&input.price, "价格")?;
    if price < Decimal::ZERO {
        return Err(CommandError::new("VALIDATION_ERROR", "价格不能为负数"));
    }
    connection.execute(r#"INSERT INTO price_points(id,instrument_id,price_date,price,source,created_at)VALUES(?1,?2,?3,?4,?5,?6)ON CONFLICT(instrument_id,price_date,source)DO UPDATE SET price=excluded.price,created_at=excluded.created_at"#,params![Uuid::now_v7().to_string(),input.instrument_id,input.price_date,decimal_string(price),input.source,now_utc()])?;
    Ok(())
}

#[derive(Default)]
struct PositionState {
    quantity: Decimal,
    total_cost: Decimal,
    realized: Decimal,
}

pub fn portfolio_snapshot(connection: &Connection) -> CommandResult<PortfolioSnapshot> {
    let instruments = list_instruments(connection)?;
    let transactions = list_transactions(connection)?;
    let mut states: HashMap<(String, String), PositionState> = HashMap::new();
    let mut ordered = transactions;
    ordered.sort_by(|a, b| {
        a.trade_date
            .cmp(&b.trade_date)
            .then(a.created_at.cmp(&b.created_at))
    });
    for tx in ordered {
        let quantity = parse_decimal(&tx.quantity, "数量")?;
        let unit_price = parse_decimal(&tx.unit_price, "单价")?;
        let explicit_amount = parse_decimal(&tx.amount, "金额")?;
        let fee = parse_decimal(&tx.fee, "手续费")?;
        let tax = parse_decimal(&tx.tax, "税费")?;
        let is_cash = instruments
            .iter()
            .any(|instrument| instrument.id == tx.instrument_id && instrument.kind == "cash");
        let state = states
            .entry((tx.account_id.clone(), tx.instrument_id.clone()))
            .or_default();
        let amount = if explicit_amount.is_zero() {
            quantity * unit_price
        } else {
            explicit_amount
        };
        match tx.kind.as_str() {
            "buy" | "subscribe" => {
                state.quantity += quantity;
                state.total_cost += amount + fee + tax;
            }
            "sell" | "redeem" => {
                if quantity > state.quantity {
                    return Err(CommandError::new(
                        "INSUFFICIENT_POSITION",
                        format!("{} 的卖出或赎回数量超过当时持仓", tx.trade_date),
                    )
                    .with_recovery("请检查交易日期、数量，或先补录更早的买入/申购记录"));
                }
                let sell_qty = quantity;
                let average = if state.quantity.is_zero() {
                    Decimal::ZERO
                } else {
                    state.total_cost / state.quantity
                };
                let allocated = average * sell_qty;
                state.quantity -= sell_qty;
                state.total_cost -= allocated;
                state.realized += amount - fee - tax - allocated;
                if state.quantity.is_zero() {
                    state.total_cost = Decimal::ZERO;
                }
            }
            "dividend" => state.realized += amount,
            "fee" | "tax" => state.realized -= amount + fee + tax,
            "adjustment" => {
                if is_cash {
                    let cash_amount = if amount.is_zero() { quantity } else { amount };
                    state.quantity += cash_amount;
                    state.total_cost += cash_amount;
                } else {
                    state.quantity += quantity;
                    state.total_cost += amount;
                }
            }
            _ => {}
        }
    }
    let mut holdings = vec![];
    let mut total_market = Decimal::ZERO;
    let mut total_cost = Decimal::ZERO;
    let mut total_realized = Decimal::ZERO;
    let mut latest_update: Option<String> = None;
    for ((account_id, instrument_id), state) in states {
        if state.quantity.is_zero() && state.realized.is_zero() {
            continue;
        }
        let Some(instrument) = instruments.iter().find(|item| item.id == instrument_id) else {
            continue;
        };
        let latest:Option<(String,String,String)>=connection.query_row("SELECT price,price_date,source FROM price_points WHERE instrument_id=?1 ORDER BY price_date DESC,created_at DESC LIMIT 1",[&instrument_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
        let (price, price_date, source) = if let Some((value, date, source)) = latest {
            (parse_decimal(&value, "行情价格")?, Some(date), source)
        } else if let Some(value) = &instrument.manual_price {
            (parse_decimal(value, "手工价格")?, None, "manual".into())
        } else if instrument.kind == "cash" {
            (Decimal::ONE, None, "manual".into())
        } else {
            (Decimal::ZERO, None, "missing".into())
        };
        if let Some(date) = &price_date
            && latest_update.as_ref().is_none_or(|current| date > current)
        {
            latest_update = Some(date.clone());
        }
        let market = state.quantity * price;
        let unrealized = market - state.total_cost;
        let average = if state.quantity.is_zero() {
            Decimal::ZERO
        } else {
            state.total_cost / state.quantity
        };
        total_market += market;
        total_cost += state.total_cost;
        total_realized += state.realized;
        holdings.push(Holding {
            account_id,
            instrument_id,
            code: instrument.code.clone(),
            name: instrument.name.clone(),
            kind: instrument.kind.clone(),
            quantity: decimal_string(state.quantity),
            average_cost: decimal_string(average),
            total_cost: decimal_string(state.total_cost),
            current_price: decimal_string(price),
            market_value: decimal_string(market),
            unrealized_gain: decimal_string(unrealized),
            realized_gain: decimal_string(state.realized),
            price_date,
            price_source: source,
        });
    }
    holdings.sort_by_key(|holding| {
        std::cmp::Reverse(parse_decimal(&holding.market_value, "市值").unwrap_or_default())
    });
    Ok(PortfolioSnapshot {
        total_market_value: decimal_string(total_market),
        total_cost: decimal_string(total_cost),
        unrealized_gain: decimal_string(total_market - total_cost),
        realized_gain: decimal_string(total_realized),
        holdings,
        updated_at: latest_update,
    })
}

fn map_review(row: &Row<'_>) -> rusqlite::Result<ReviewSnapshot> {
    Ok(ReviewSnapshot {
        id: row.get(0)?,
        period_type: row.get(1)?,
        start_date: row.get(2)?,
        end_date: row.get(3)?,
        summary: row.get(4)?,
        reflection: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}
pub fn list_reviews(connection: &Connection) -> CommandResult<Vec<ReviewSnapshot>> {
    let mut s=connection.prepare("SELECT id,period_type,start_date,end_date,summary,reflection,status,created_at,updated_at FROM review_snapshots ORDER BY end_date DESC")?;
    let r = s.query_map([], map_review)?;
    Ok(r.collect::<Result<Vec<_>, _>>()?)
}
pub fn upsert_review(
    connection: &Connection,
    mut item: ReviewSnapshot,
) -> CommandResult<ReviewSnapshot> {
    item.id = identifier(&item.id);
    require_choice(&item.period_type, "复盘周期", &["week", "month"])?;
    require_choice(&item.status, "复盘状态", &["draft", "confirmed"])?;
    let start = parse_date(&item.start_date, "复盘开始日期")?;
    let end = parse_date(&item.end_date, "复盘结束日期")?;
    if end < start {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "复盘结束日期不能早于开始日期",
        ));
    }
    validate_text_length(&item.summary, "复盘摘要", 1_000_000)?;
    validate_text_length(&item.reflection, "复盘结论", 1_000_000)?;
    let now = now_utc();
    let created = if item.created_at.is_empty() {
        now.clone()
    } else {
        item.created_at.clone()
    };
    connection.execute(r#"INSERT INTO review_snapshots(id,period_type,start_date,end_date,summary,reflection,status,created_at,updated_at)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)ON CONFLICT(id)DO UPDATE SET period_type=excluded.period_type,start_date=excluded.start_date,end_date=excluded.end_date,summary=excluded.summary,reflection=excluded.reflection,status=excluded.status,updated_at=excluded.updated_at"#,params![item.id,item.period_type,item.start_date,item.end_date,item.summary,item.reflection,item.status,created,now])?;
    connection.query_row("SELECT id,period_type,start_date,end_date,summary,reflection,status,created_at,updated_at FROM review_snapshots WHERE id=?1",[&item.id],map_review).map_err(Into::into)
}

pub fn delete_record(connection: &Connection, kind: &str, id: &str) -> CommandResult<()> {
    let table = match kind {
        "task" => "tasks",
        "calendar" => "calendar_items",
        "project" => "projects",
        "worklog" => "work_logs",
        "goal" => "goals",
        "habit" => "habits",
        "learning" => "learning_items",
        "content" => "content_items",
        "account" => "investment_accounts",
        "instrument" => "instruments",
        "transaction" => "portfolio_transactions",
        "review" => "review_snapshots",
        _ => {
            return Err(CommandError::new(
                "INVALID_RECORD_KIND",
                "不支持删除此类型的记录",
            ));
        }
    };
    let affected = connection.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id])?;
    if affected == 0 {
        return Err(CommandError::new("NOT_FOUND", "未找到要删除的记录"));
    }
    Ok(())
}

pub fn settings(connection: &Connection) -> CommandResult<AppSettings> {
    load_settings(connection)
}
pub fn update_settings(connection: &Connection, value: AppSettings) -> CommandResult<AppSettings> {
    save_settings(connection, &value)?;
    load_settings(connection)
}

fn shanghai_calendar_date(timestamp: &str) -> Option<String> {
    let offset = FixedOffset::east_opt(8 * 60 * 60)?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&offset).format("%Y-%m-%d").to_string())
}

fn shanghai_datetime(timestamp: &str) -> Option<NaiveDateTime> {
    let offset = FixedOffset::east_opt(8 * 60 * 60)?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&offset).naive_local())
        .or_else(|| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S").ok())
        .or_else(|| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M").ok())
}

pub fn dashboard(connection: &Connection, date: &str) -> CommandResult<Dashboard> {
    let today = parse_date(date, "首页日期")?;
    let all_tasks = list_tasks(connection)?;
    let mut tasks = all_tasks
        .iter()
        .filter(|task| {
            task.status != "done"
                && (task.due_date.as_deref().is_none_or(|due| due <= date)
                    || task
                        .scheduled_start
                        .as_deref()
                        .is_some_and(|start| start.starts_with(date)))
        })
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    tasks.extend(
        all_tasks
            .into_iter()
            .filter(|task| {
                task.status == "done"
                    && task
                        .completed_at
                        .as_deref()
                        .and_then(shanghai_calendar_date)
                        .is_some_and(|completed_date| completed_date == date)
            })
            .take(3),
    );
    let shanghai_offset = FixedOffset::east_opt(8 * 60 * 60)
        .ok_or_else(|| CommandError::new("INVALID_TIMEZONE", "无法初始化上海时区"))?;
    let shanghai_now = Utc::now().with_timezone(&shanghai_offset).naive_local();
    let is_current_day = today == shanghai_now.date();
    let mut upcoming = list_calendar(connection, None, None)?
        .into_iter()
        .filter_map(|item| next_calendar_occurrence(item, today))
        .filter(|item| {
            !is_current_day
                || item.all_day
                || shanghai_datetime(&item.end_at).is_none_or(|end| end >= shanghai_now)
        })
        .collect::<Vec<_>>();
    upcoming.sort_by(|left, right| left.start_at.cmp(&right.start_at));
    let next_event = upcoming.into_iter().next();
    let habits = list_habits(connection, date)?;
    let work_logs = list_work_logs(connection)?
        .into_iter()
        .filter(|log| log.log_date == date)
        .take(4)
        .collect();
    let content_items = list_content(connection)?
        .into_iter()
        .filter(|item| item.status != "archived" && item.status != "published")
        .take(5)
        .collect();
    Ok(Dashboard {
        date: date.into(),
        tasks,
        next_event,
        habits,
        work_logs,
        content_items,
        portfolio: portfolio_snapshot(connection)?,
        settings: load_settings(connection)?,
    })
}

fn recurrence_occurs(start: NaiveDate, target: NaiveDate, recurrence: &str) -> bool {
    if target < start {
        return false;
    }
    match recurrence {
        "daily" => true,
        "weekly" => start.weekday() == target.weekday(),
        "monthly" => start.day() == target.day(),
        _ => start == target,
    }
}

fn next_calendar_occurrence(item: CalendarItem, from: NaiveDate) -> Option<CalendarItem> {
    let start_date = NaiveDate::parse_from_str(item.start_at.get(..10)?, "%Y-%m-%d").ok()?;
    let end_date = NaiveDate::parse_from_str(item.end_at.get(..10)?, "%Y-%m-%d").ok()?;
    let duration_days = (end_date - start_date).num_days().max(0);
    if item.recurrence == "none" {
        let end_date = item.end_at.get(..10).unwrap_or_default();
        return (end_date >= from.format("%Y-%m-%d").to_string().as_str()).then_some(item);
    }
    for offset in 0..=366 {
        let target = from + Duration::days(offset);
        if recurrence_occurs(start_date, target, &item.recurrence) {
            let mut occurrence = item;
            occurrence.start_at = format!(
                "{}{}",
                target.format("%Y-%m-%d"),
                occurrence.start_at.get(10..).unwrap_or("")
            );
            occurrence.end_at = format!(
                "{}{}",
                (target + Duration::days(duration_days)).format("%Y-%m-%d"),
                occurrence.end_at.get(10..).unwrap_or("")
            );
            return Some(occurrence);
        }
    }
    None
}

pub fn search(connection: &Connection, query: &str) -> CommandResult<Vec<SearchResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    if q.chars().count() > 240 {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "搜索关键词不能超过 240 个字符",
        ));
    }
    let pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let mut results = vec![];
    let specs = [
        ("task", "tasks", "title", "COALESCE(due_date,'无截止日期')"),
        ("project", "projects", "name", "area"),
        ("worklog", "work_logs", "title", "log_date"),
        ("goal", "goals", "title", "horizon"),
        ("learning", "learning_items", "title", "kind"),
        ("content", "content_items", "title", "platform"),
        ("instrument", "instruments", "name", "code"),
    ];
    for (kind, table, title, subtitle) in specs {
        let sql = format!(
            "SELECT id,{title},{subtitle} FROM {table} WHERE {title} LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 6"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([&pattern], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                kind: kind.into(),
                title: row.get(1)?,
                subtitle: row.get(2)?,
            })
        })?;
        results.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    results.truncate(24);
    Ok(results)
}

pub fn weekly_summary(
    connection: &Connection,
    start_date: &str,
    end_date: &str,
) -> CommandResult<String> {
    let start = parse_date(start_date, "复盘开始日期")?;
    let end = parse_date(end_date, "复盘结束日期")?;
    if end < start {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            "复盘结束日期不能早于开始日期",
        ));
    }
    let completed = list_tasks(connection)?
        .into_iter()
        .filter(|task| {
            task.status == "done"
                && task
                    .completed_at
                    .as_deref()
                    .and_then(shanghai_calendar_date)
                    .is_some_and(|date| date.as_str() >= start_date && date.as_str() <= end_date)
        })
        .count();
    let habit_checks: i64 = connection.query_row(
        "SELECT count(*) FROM habit_checks WHERE check_date BETWEEN ?1 AND ?2 AND value>0",
        params![start_date, end_date],
        |r| r.get(0),
    )?;
    let work_minutes: i64 = connection.query_row(
        "SELECT COALESCE(sum(minutes),0) FROM work_logs WHERE log_date BETWEEN ?1 AND ?2",
        params![start_date, end_date],
        |r| r.get(0),
    )?;
    let published: i64 = connection.query_row(
        "SELECT count(*) FROM content_items WHERE status IN('published','review') AND COALESCE(NULLIF(substr(publish_at,1,10),''), substr(updated_at,1,10)) BETWEEN ?1 AND ?2",
        params![start_date, end_date],
        |row| row.get(0),
    )?;
    let portfolio = portfolio_snapshot(connection)?;
    Ok(format!(
        "本周完成 {completed} 项任务，累计习惯打卡 {habit_checks} 次，记录工作 {} 小时，发布内容 {published} 条。当前投资组合市值 ¥{}，未实现收益 ¥{}。",
        Decimal::from(work_minutes) / Decimal::from(60),
        portfolio.total_market_value,
        portfolio.unrealized_gain
    ))
}

pub fn is_saved_file_reference(connection: &Connection, path: &str) -> CommandResult<bool> {
    if path.trim().is_empty() {
        return Ok(false);
    }
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM work_logs WHERE attachment_path = ?1
                UNION ALL
                SELECT 1 FROM content_items WHERE asset_path = ?1
            )",
            [path],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::open_database,
        models::{Instrument, InvestmentAccount, PortfolioTransaction},
    };

    #[allow(clippy::too_many_arguments)]
    fn transaction(
        account_id: &str,
        instrument_id: &str,
        kind: &str,
        date: &str,
        quantity: &str,
        unit_price: &str,
        amount: &str,
        fee: &str,
        tax: &str,
    ) -> PortfolioTransaction {
        PortfolioTransaction {
            id: String::new(),
            account_id: account_id.into(),
            instrument_id: instrument_id.into(),
            kind: kind.into(),
            trade_date: date.into(),
            quantity: quantity.into(),
            unit_price: unit_price.into(),
            amount: amount.into(),
            fee: fee.into(),
            tax: tax.into(),
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn decimal_values_remain_exact() {
        let quantity = Decimal::from_str("123.4567").unwrap();
        let price = Decimal::from_str("1.2345").unwrap();
        assert_eq!(decimal_string(quantity * price), "152.40729615");
    }

    #[test]
    fn weighted_average_cost_math_is_exact() {
        let mut state = PositionState::default();
        state.quantity += Decimal::from(100);
        state.total_cost += Decimal::from(1000);
        state.quantity += Decimal::from(100);
        state.total_cost += Decimal::from(1200);
        let average = state.total_cost / state.quantity;
        let sold = Decimal::from(50);
        state.quantity -= sold;
        state.total_cost -= average * sold;
        assert_eq!(average, Decimal::from(11));
        assert_eq!(state.total_cost, Decimal::from(1650));
    }

    #[test]
    fn recurrence_rules_respect_start_date_and_calendar_unit() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        assert!(recurrence_occurs(
            start,
            NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
            "daily"
        ));
        assert!(recurrence_occurs(
            start,
            NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
            "weekly"
        ));
        assert!(!recurrence_occurs(
            start,
            NaiveDate::from_ymd_opt(2026, 8, 11).unwrap(),
            "weekly"
        ));
        assert!(recurrence_occurs(
            start,
            NaiveDate::from_ymd_opt(2026, 9, 3).unwrap(),
            "monthly"
        ));
        assert!(!recurrence_occurs(
            start,
            NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
            "daily"
        ));
    }

    #[test]
    fn recurring_calendar_preserves_cross_day_duration() {
        let occurrence = next_calendar_occurrence(
            CalendarItem {
                id: "overnight".into(),
                kind: "event".into(),
                title: "跨日日程".into(),
                notes: String::new(),
                start_at: "2026-08-03T23:00".into(),
                end_at: "2026-08-04T01:00".into(),
                all_day: false,
                recurrence: "weekly".into(),
                source: "internal".into(),
                external_uid: None,
                project_id: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
        )
        .unwrap();
        assert_eq!(occurrence.start_at, "2026-08-10T23:00");
        assert_eq!(occurrence.end_at, "2026-08-11T01:00");
    }

    #[test]
    fn cash_adjustment_uses_amount_and_has_unit_price() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("cash.sqlite3"), &[8_u8; 32]).unwrap();
        let account = upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "现金账户".into(),
                kind: "cash".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let instrument = upsert_instrument(
            &connection,
            Instrument {
                id: String::new(),
                code: "CNY".into(),
                name: "人民币现金".into(),
                kind: "cash".into(),
                market: "CN".into(),
                currency: "CNY".into(),
                manual_price: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        upsert_transaction(
            &connection,
            transaction(
                &account.id,
                &instrument.id,
                "adjustment",
                "2026-08-31",
                "0",
                "0",
                "1000.25",
                "0",
                "0",
            ),
        )
        .unwrap();

        let snapshot = portfolio_snapshot(&connection).unwrap();
        let holding = snapshot.holdings.first().unwrap();
        assert_eq!(holding.quantity, "1000.25");
        assert_eq!(holding.current_price, "1");
        assert_eq!(holding.market_value, "1000.25");
        assert_eq!(holding.total_cost, "1000.25");
    }

    #[test]
    fn invalid_dates_and_enum_values_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("validation.sqlite3"), &[6_u8; 32]).unwrap();
        assert_eq!(
            dashboard(&connection, "2026-02-30").unwrap_err().code,
            "VALIDATION_ERROR"
        );
        let invalid_account = upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "错误账户".into(),
                kind: "unknown".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap_err();
        assert_eq!(invalid_account.code, "VALIDATION_ERROR");
    }

    #[test]
    fn portfolio_engine_handles_sales_dividends_fees_and_manual_prices() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("portfolio.sqlite3"), &[9_u8; 32]).unwrap();
        let account = upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "主账户".into(),
                kind: "securities".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let instrument = upsert_instrument(
            &connection,
            Instrument {
                id: String::new(),
                code: "510300".into(),
                name: "沪深300ETF".into(),
                kind: "etf".into(),
                market: "CN".into(),
                currency: "CNY".into(),
                manual_price: Some("14".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();

        let rows = [
            transaction(
                &account.id,
                &instrument.id,
                "buy",
                "2026-01-01",
                "100",
                "10",
                "0",
                "1",
                "0",
            ),
            transaction(
                &account.id,
                &instrument.id,
                "buy",
                "2026-01-02",
                "100",
                "12",
                "0",
                "1",
                "0",
            ),
            transaction(
                &account.id,
                &instrument.id,
                "sell",
                "2026-01-03",
                "50",
                "15",
                "0",
                "2",
                "1",
            ),
            transaction(
                &account.id,
                &instrument.id,
                "dividend",
                "2026-01-04",
                "0",
                "0",
                "60",
                "0",
                "0",
            ),
            transaction(
                &account.id,
                &instrument.id,
                "fee",
                "2026-01-05",
                "0",
                "0",
                "10",
                "0",
                "0",
            ),
        ];
        for row in rows {
            upsert_transaction(&connection, row).unwrap();
        }

        let snapshot = portfolio_snapshot(&connection).unwrap();
        let holding = snapshot.holdings.first().unwrap();
        assert_eq!(holding.quantity, "150");
        assert_eq!(holding.average_cost, "11.01");
        assert_eq!(holding.total_cost, "1651.5");
        assert_eq!(holding.market_value, "2100");
        assert_eq!(holding.unrealized_gain, "448.5");
        assert_eq!(holding.realized_gain, "246.5");
        assert_eq!(holding.price_source, "manual");
    }

    #[test]
    fn portfolio_rejects_oversell_and_rolls_back_invalid_transaction() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("oversell.sqlite3"), &[11_u8; 32]).unwrap();
        let account = upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "测试账户".into(),
                kind: "securities".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let instrument = upsert_instrument(
            &connection,
            Instrument {
                id: String::new(),
                code: "510300".into(),
                name: "测试 ETF".into(),
                kind: "etf".into(),
                market: "CN".into(),
                currency: "CNY".into(),
                manual_price: Some("4".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        upsert_transaction(
            &connection,
            transaction(
                &account.id,
                &instrument.id,
                "buy",
                "2026-01-01",
                "10",
                "4",
                "0",
                "0",
                "0",
            ),
        )
        .unwrap();
        let result = upsert_transaction(
            &connection,
            transaction(
                &account.id,
                &instrument.id,
                "sell",
                "2026-01-02",
                "11",
                "5",
                "0",
                "0",
                "0",
            ),
        );
        assert_eq!(result.unwrap_err().code, "INSUFFICIENT_POSITION");
        assert_eq!(list_transactions(&connection).unwrap().len(), 1);
    }

    #[test]
    fn dashboard_counts_completed_tasks_by_shanghai_calendar_day() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("dashboard.sqlite3"), &[12_u8; 32]).unwrap();
        connection.execute(
            "INSERT INTO tasks(id,title,notes,status,priority,due_date,scheduled_start,scheduled_end,recurrence,project_id,goal_id,content_id,completed_at,created_at,updated_at) VALUES('done-task','跨午夜完成','','done',2,'2026-08-29',NULL,NULL,'none',NULL,NULL,NULL,'2026-08-29T16:30:00Z','2026-08-29T16:00:00Z','2026-08-29T16:30:00Z')",
            [],
        ).unwrap();
        let result = dashboard(&connection, "2026-08-30").unwrap();
        assert!(result.tasks.iter().any(|task| task.id == "done-task"));
    }

    #[test]
    fn completing_repeating_task_keeps_next_occurrence_and_history() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("recurring.sqlite3"), &[14_u8; 32]).unwrap();
        connection.execute(
            "INSERT INTO tasks(id,title,notes,status,priority,due_date,scheduled_start,scheduled_end,recurrence,project_id,goal_id,content_id,completed_at,created_at,updated_at) VALUES('repeat-task','每日复盘','','todo',2,'2026-08-29','2026-08-29T20:00','2026-08-29T20:30','daily',NULL,NULL,NULL,NULL,'2026-08-01T00:00:00Z','2026-08-01T00:00:00Z')",
            [],
        ).unwrap();

        let next = toggle_task(&connection, "repeat-task", true).unwrap();
        assert_eq!(next.status, "todo");
        assert_eq!(next.due_date.as_deref(), Some("2026-08-30"));
        assert_eq!(next.scheduled_start.as_deref(), Some("2026-08-30T20:00"));

        let tasks = list_tasks(&connection).unwrap();
        let history = tasks.iter().find(|task| task.id != "repeat-task").unwrap();
        assert_eq!(history.status, "done");
        assert_eq!(history.recurrence, "none");
        assert_eq!(history.due_date.as_deref(), Some("2026-08-29"));
    }

    #[test]
    fn shanghai_datetime_converts_utc_before_comparing_calendar_items() {
        let parsed = shanghai_datetime("2026-08-29T16:30:00Z").unwrap();
        assert_eq!(
            parsed.format("%Y-%m-%d %H:%M").to_string(),
            "2026-08-30 00:30"
        );
        assert_eq!(
            shanghai_datetime("2026-08-30T09:15")
                .unwrap()
                .format("%H:%M")
                .to_string(),
            "09:15"
        );
    }

    #[test]
    fn weekly_summary_counts_content_by_publish_date() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            open_database(&directory.path().join("review.sqlite3"), &[15_u8; 32]).unwrap();
        connection.execute(
            "INSERT INTO content_items(id,title,platform,format,status,goal,tags_json,publish_at,notes,asset_path,created_at,updated_at) VALUES('published','周报','公众号','图文','published','','[]','2026-08-27T10:00','',NULL,'2026-07-01T00:00:00Z','2026-08-01T00:00:00Z')",
            [],
        ).unwrap();
        let summary = weekly_summary(&connection, "2026-08-24", "2026-08-30").unwrap();
        assert!(summary.contains("发布内容 1 条"));
    }

    #[test]
    fn portfolio_rejects_negative_financial_values() {
        let connection = Connection::open_in_memory().unwrap();
        let result = upsert_transaction(
            &connection,
            transaction("a", "i", "buy", "2026-01-01", "-1", "10", "0", "0", "0"),
        );
        let error = result.unwrap_err();
        assert_eq!(error.code, "VALIDATION_ERROR");
    }

    #[test]
    fn full_workspace_round_trip_preserves_every_domain_and_relation() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("workspace-round-trip.sqlite3");
        let key = [21_u8; 32];
        let connection = open_database(&database_path, &key).unwrap();

        let project = upsert_project(
            &connection,
            Project {
                id: String::new(),
                name: "端到端项目".into(),
                area: "work".into(),
                status: "active".into(),
                color: "#397064".into(),
                notes: "验证跨模块关系".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let goal = upsert_goal(
            &connection,
            Goal {
                id: String::new(),
                title: "端到端成长目标".into(),
                horizon: "quarter".into(),
                status: "active".into(),
                progress: 35,
                notes: "持续验证".into(),
                start_date: Some("2026-08-01".into()),
                target_date: Some("2026-09-30".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let content = upsert_content(
            &connection,
            ContentItem {
                id: String::new(),
                title: "端到端内容".into(),
                platform: "公众号".into(),
                format: "图文".into(),
                status: "published".into(),
                goal: "记录开发过程".into(),
                tags: vec!["验证".into(), "本地优先".into()],
                publish_at: Some("2026-08-30T10:00".into()),
                notes: "内容备注".into(),
                asset_path: Some("/tmp/reference.md".into()),
                views: 0,
                likes: 0,
                comments: 0,
                saves: 0,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        add_content_metric(
            &connection,
            ContentMetricInput {
                content_id: content.id.clone(),
                recorded_at: "2026-08-30T20:00:00+08:00".into(),
                views: 1200,
                likes: 80,
                comments: 12,
                saves: 33,
                followers_delta: 5,
            },
        )
        .unwrap();
        let task = upsert_task(
            &connection,
            Task {
                id: String::new(),
                title: "端到端关联任务".into(),
                notes: "同时关联项目、目标和内容".into(),
                status: "todo".into(),
                priority: 1,
                due_date: Some("2026-08-30".into()),
                scheduled_start: Some("2026-08-30T09:00".into()),
                scheduled_end: Some("2026-08-30T10:00".into()),
                recurrence: "none".into(),
                project_id: Some(project.id.clone()),
                goal_id: Some(goal.id.clone()),
                content_id: Some(content.id.clone()),
                completed_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        upsert_calendar(
            &connection,
            CalendarItem {
                id: String::new(),
                kind: "timeblock".into(),
                title: "端到端专注时段".into(),
                notes: "保护关键工作时间".into(),
                start_at: "2026-08-30T09:00:00+08:00".into(),
                end_at: "2026-08-30T10:00:00+08:00".into(),
                all_day: false,
                recurrence: "weekly".into(),
                source: "internal".into(),
                external_uid: None,
                project_id: Some(project.id.clone()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        upsert_work_log(
            &connection,
            WorkLog {
                id: String::new(),
                log_date: "2026-08-30".into(),
                project_id: Some(project.id.clone()),
                title: "端到端工作记录".into(),
                completed: "完成持久化验证".into(),
                blockers: "无".into(),
                next_steps: "运行打包测试".into(),
                minutes: 75,
                energy: 4,
                markdown: "## 验证\n数据应在重新打开后存在。".into(),
                attachment_path: Some("/tmp/evidence.txt".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let habit = upsert_habit(
            &connection,
            Habit {
                id: String::new(),
                name: "端到端习惯".into(),
                frequency: "daily".into(),
                target_per_week: 7,
                color: "#397064".into(),
                active: true,
                checked_today: false,
                streak: 0,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        check_habit(&connection, &habit.id, "2026-08-30", true).unwrap();
        upsert_learning(
            &connection,
            LearningItem {
                id: String::new(),
                title: "端到端学习项目".into(),
                kind: "project".into(),
                status: "learning".into(),
                progress: 45,
                notes: "记录验证方法".into(),
                target_date: Some("2026-09-15".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let account = upsert_account(
            &connection,
            InvestmentAccount {
                id: String::new(),
                name: "端到端证券账户".into(),
                kind: "securities".into(),
                currency: "CNY".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let instrument = upsert_instrument(
            &connection,
            Instrument {
                id: String::new(),
                code: "510300".into(),
                name: "端到端 ETF".into(),
                kind: "etf".into(),
                market: "CN".into(),
                currency: "CNY".into(),
                manual_price: Some("4.125".into()),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        upsert_transaction(
            &connection,
            transaction(
                &account.id,
                &instrument.id,
                "buy",
                "2026-08-30",
                "10",
                "4.125",
                "0",
                "1",
                "0",
            ),
        )
        .unwrap();
        upsert_price(
            &connection,
            PricePointInput {
                instrument_id: instrument.id.clone(),
                price_date: "2026-08-30".into(),
                price: "4.5".into(),
                source: "manual".into(),
            },
        )
        .unwrap();
        upsert_review(
            &connection,
            ReviewSnapshot {
                id: String::new(),
                period_type: "week".into(),
                start_date: "2026-08-24".into(),
                end_date: "2026-08-30".into(),
                summary: "端到端摘要".into(),
                reflection: "端到端复盘结论".into(),
                status: "confirmed".into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        update_settings(
            &connection,
            AppSettings {
                theme: "dark".into(),
                amounts_hidden: true,
                lock_minutes: 30,
                quote_enabled: false,
                quote_auto_refresh: false,
                last_quote_refresh: None,
                timezone: "Asia/Shanghai".into(),
                currency: "CNY".into(),
            },
        )
        .unwrap();

        let dashboard_before = dashboard(&connection, "2026-08-30").unwrap();
        assert!(dashboard_before.tasks.iter().any(|item| item.id == task.id));
        assert!(dashboard_before.habits[0].checked_today);
        assert!(search(&connection, "端到端").unwrap().len() >= 7);
        assert!(
            weekly_summary(&connection, "2026-08-24", "2026-08-30")
                .unwrap()
                .contains("发布内容 1 条")
        );
        drop(connection);

        let reopened = open_database(&database_path, &key).unwrap();
        let saved_task = list_tasks(&reopened)
            .unwrap()
            .into_iter()
            .find(|item| item.id == task.id)
            .unwrap();
        assert_eq!(saved_task.project_id.as_deref(), Some(project.id.as_str()));
        assert_eq!(saved_task.goal_id.as_deref(), Some(goal.id.as_str()));
        assert_eq!(saved_task.content_id.as_deref(), Some(content.id.as_str()));
        assert_eq!(list_calendar(&reopened, None, None).unwrap().len(), 1);
        assert_eq!(list_work_logs(&reopened).unwrap()[0].minutes, 75);
        assert_eq!(list_goals(&reopened).unwrap()[0].progress, 35);
        assert!(list_habits(&reopened, "2026-08-30").unwrap()[0].checked_today);
        assert_eq!(list_learning(&reopened).unwrap()[0].progress, 45);
        assert_eq!(list_content(&reopened).unwrap()[0].views, 1200);
        assert_eq!(list_accounts(&reopened).unwrap().len(), 1);
        assert_eq!(list_instruments(&reopened).unwrap().len(), 1);
        assert_eq!(list_transactions(&reopened).unwrap().len(), 1);
        let snapshot = portfolio_snapshot(&reopened).unwrap();
        assert_eq!(snapshot.holdings[0].current_price, "4.5");
        assert_eq!(snapshot.holdings[0].market_value, "45");
        assert_eq!(snapshot.holdings[0].unrealized_gain, "2.75");
        assert_eq!(list_reviews(&reopened).unwrap()[0].status, "confirmed");
        assert_eq!(settings(&reopened).unwrap().theme, "dark");
        crate::database::validate_connection(&reopened).unwrap();
    }

    #[test]
    #[ignore = "本机性能验收；使用 cargo test performance_acceptance -- --ignored 运行"]
    fn performance_acceptance_with_large_local_dataset() {
        use std::time::{Duration as StdDuration, Instant};

        let directory = tempfile::tempdir().unwrap();
        let mut connection =
            open_database(&directory.path().join("performance.sqlite3"), &[13_u8; 32]).unwrap();
        let transaction = connection.transaction().unwrap();
        {
            let mut task_insert = transaction.prepare("INSERT INTO tasks(id,title,notes,status,priority,due_date,scheduled_start,scheduled_end,recurrence,project_id,goal_id,content_id,completed_at,created_at,updated_at) VALUES(?1,?2,'','todo',2,'2026-08-29',NULL,NULL,'none',NULL,NULL,NULL,NULL,'2026-08-29T00:00:00Z','2026-08-29T00:00:00Z')").unwrap();
            for index in 0..10_000 {
                task_insert
                    .execute(params![format!("task-{index}"), format!("任务 {index}")])
                    .unwrap();
            }
        }
        transaction.execute(
            "UPDATE tasks SET status='done', completed_at='2026-08-29T12:00:00Z' WHERE id IN ('task-0','task-1','task-2')",
            [],
        ).unwrap();
        {
            let mut log_insert = transaction.prepare("INSERT INTO work_logs(id,log_date,project_id,title,completed,blockers,next_steps,minutes,energy,markdown,attachment_path,created_at,updated_at) VALUES(?1,'2026-08-29',NULL,?2,'完成','','下一步',30,3,'',NULL,'2026-08-29T00:00:00Z','2026-08-29T00:00:00Z')").unwrap();
            for index in 0..5_000 {
                log_insert
                    .execute(params![format!("log-{index}"), format!("日志 {index}")])
                    .unwrap();
            }
        }
        transaction.execute("INSERT INTO instruments(id,code,name,kind,market,currency,manual_price,created_at,updated_at) VALUES('instrument-perf','510300','性能测试ETF','etf','CN','CNY','4.2','2026-08-29T00:00:00Z','2026-08-29T00:00:00Z')", []).unwrap();
        {
            let mut price_insert = transaction.prepare("INSERT INTO price_points(id,instrument_id,price_date,price,source,created_at) VALUES(?1,'instrument-perf',?2,'4.2','manual','2026-08-29T00:00:00Z')").unwrap();
            for index in 0..50_000 {
                price_insert
                    .execute(params![
                        format!("price-{index}"),
                        format!("point-{index:05}")
                    ])
                    .unwrap();
            }
        }
        transaction.commit().unwrap();

        let dashboard_started = Instant::now();
        let result = dashboard(&connection, "2026-08-29").unwrap();
        let dashboard_elapsed = dashboard_started.elapsed();
        assert_eq!(result.tasks.len(), 8);
        assert!(
            dashboard_elapsed < StdDuration::from_millis(300),
            "首页聚合耗时 {dashboard_elapsed:?}"
        );

        let search_started = Instant::now();
        let results = search(&connection, "任务 999").unwrap();
        let search_elapsed = search_started.elapsed();
        assert!(!results.is_empty());
        assert!(
            search_elapsed < StdDuration::from_millis(200),
            "搜索耗时 {search_elapsed:?}"
        );
        eprintln!("dashboard={dashboard_elapsed:?}, search={search_elapsed:?}");
    }
}
