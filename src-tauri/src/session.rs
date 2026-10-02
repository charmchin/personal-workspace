//! Authorization is independent of the database mutex so native callbacks never wait on SQL.
use crate::error::{CommandError, CommandResult};
use serde::Serialize;
use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum SystemEvent {
    ScreenLocked,
    ScreenUnlocked,
    SessionInactive,
    SessionActive,
    Sleeping,
    Woke,
    DisplaySleeping,
    DisplayWoke,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSnapshot {
    pub session_epoch: u64,
    pub unlocked: bool,
    pub lock_reason: Option<String>,
}

struct Policy {
    epoch: u64,
    active: bool,
    blocked: u8,
    timeout: Option<Duration>,
    last_activity: Instant,
    reason: Option<String>,
}

impl Policy {
    fn revoke(&mut self, reason: &str) {
        self.epoch += 1;
        self.active = false;
        self.reason = Some(reason.into());
    }

    fn expire(&mut self, now: Instant) {
        if self.active
            && self
                .timeout
                .is_some_and(|timeout| now.saturating_duration_since(self.last_activity) >= timeout)
        {
            self.revoke("idle");
        }
    }

    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            session_epoch: self.epoch,
            unlocked: self.active,
            lock_reason: self.reason.clone(),
        }
    }
}

pub(crate) struct SessionPolicy(Mutex<Policy>);

impl SessionPolicy {
    pub fn new() -> Self {
        Self(Mutex::new(Policy {
            epoch: 0,
            active: false,
            blocked: 0,
            timeout: Some(Duration::from_secs(15 * 60)),
            last_activity: Instant::now(),
            reason: None,
        }))
    }

    fn policy(&self) -> MutexGuard<'_, Policy> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn challenge(&self) -> CommandResult<u64> {
        let mut policy = self.policy();
        policy.expire(Instant::now());
        if policy.blocked != 0 {
            return Err(CommandError::new(
                "SYSTEM_SESSION_INACTIVE",
                "系统会话尚未恢复，不能解锁工作台",
            )
            .with_recovery("请先解锁 Mac、唤醒屏幕并返回当前用户会话。"));
        }
        Ok(policy.epoch)
    }

    pub fn activate(&self, ticket: u64, minutes: i64) -> CommandResult<u64> {
        self.activate_at(ticket, minutes, Instant::now())
    }

    fn activate_at(&self, ticket: u64, minutes: i64, now: Instant) -> CommandResult<u64> {
        let timeout = timeout(minutes)?;
        let mut policy = self.policy();
        if policy.epoch != ticket || policy.blocked != 0 {
            return Err(CommandError::locked());
        }
        policy.epoch += 1;
        policy.active = true;
        policy.reason = None;
        policy.last_activity = now;
        policy.timeout = timeout;
        Ok(policy.epoch)
    }

    pub fn require_active(&self) -> CommandResult<u64> {
        let snapshot = self.snapshot();
        if !snapshot.unlocked {
            return Err(CommandError::locked());
        }
        Ok(snapshot.session_epoch)
    }

    pub fn require_epoch(&self, epoch: u64) -> CommandResult<()> {
        let snapshot = self.snapshot();
        if !snapshot.unlocked || snapshot.session_epoch != epoch {
            return Err(CommandError::locked());
        }
        Ok(())
    }

    pub fn activity(&self) -> CommandResult<()> {
        self.activity_at(Instant::now())
    }

    fn activity_at(&self, now: Instant) -> CommandResult<()> {
        let mut policy = self.policy();
        policy.expire(now);
        if !policy.active {
            return Err(CommandError::locked());
        }
        policy.last_activity = now;
        Ok(())
    }

    pub fn configure(&self, minutes: i64) -> CommandResult<()> {
        let value = timeout(minutes)?;
        let mut policy = self.policy();
        policy.expire(Instant::now());
        if !policy.active {
            return Err(CommandError::locked());
        }
        policy.timeout = value;
        policy.expire(Instant::now());
        if !policy.active {
            return Err(CommandError::locked());
        }
        Ok(())
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.snapshot_at(Instant::now())
    }

    fn snapshot_at(&self, now: Instant) -> SessionSnapshot {
        let mut policy = self.policy();
        policy.expire(now);
        policy.snapshot()
    }

    pub fn revoke(&self, reason: &str) -> SessionSnapshot {
        let mut policy = self.policy();
        policy.revoke(reason);
        policy.snapshot()
    }

    pub fn system_event(&self, event: SystemEvent) -> SessionSnapshot {
        let (bit, entering) = match event {
            SystemEvent::ScreenLocked => (1, true),
            SystemEvent::ScreenUnlocked => (1, false),
            SystemEvent::SessionInactive => (2, true),
            SystemEvent::SessionActive => (2, false),
            SystemEvent::Sleeping => (4, true),
            SystemEvent::Woke => (4, false),
            SystemEvent::DisplaySleeping => (8, true),
            SystemEvent::DisplayWoke => (8, false),
        };
        let mut policy = self.policy();
        if entering {
            policy.blocked |= bit;
        } else {
            policy.blocked &= !bit;
        }
        // Returning to macOS never unlocks the workbench, and invalidates in-flight authentication.
        policy.revoke(if entering { "system" } else { "systemResume" });
        policy.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn age_for_test(&self, seconds: u64) {
        self.policy().last_activity -= Duration::from_secs(seconds);
    }
}

fn timeout(minutes: i64) -> CommandResult<Option<Duration>> {
    match minutes {
        0 => Ok(None),
        5 | 15 | 30 => Ok(Some(Duration::from_secs(minutes as u64 * 60))),
        _ => Err(CommandError::new("INVALID_LOCK_TIME", "自动锁定时间无效")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_deadline_expires_without_frontend_and_late_activity_cannot_revive_it() {
        let policy = SessionPolicy::new();
        let now = Instant::now();
        policy.activate_at(0, 5, now).unwrap();
        assert!(policy.snapshot_at(now + Duration::from_secs(299)).unlocked);
        assert!(!policy.snapshot_at(now + Duration::from_secs(300)).unlocked);
        assert_eq!(
            policy
                .activity_at(now + Duration::from_secs(301))
                .unwrap_err()
                .code,
            "APP_LOCKED"
        );
    }

    #[test]
    fn only_user_activity_extends_deadline_and_never_setting_disables_only_idle() {
        let policy = SessionPolicy::new();
        let now = Instant::now();
        policy.activate_at(0, 5, now).unwrap();
        policy.activity_at(now + Duration::from_secs(299)).unwrap();
        assert!(policy.snapshot_at(now + Duration::from_secs(598)).unlocked);
        assert!(!policy.snapshot_at(now + Duration::from_secs(599)).unlocked);
        policy
            .activate_at(policy.challenge().unwrap(), 0, now)
            .unwrap();
        assert!(
            policy
                .snapshot_at(now + Duration::from_secs(86400))
                .unlocked
        );
        policy.system_event(SystemEvent::ScreenLocked);
        assert!(!policy.snapshot().unlocked);
    }

    #[test]
    fn system_lock_cancels_pending_unlock_and_resume_never_unlocks_automatically() {
        let policy = SessionPolicy::new();
        let ticket = policy.challenge().unwrap();
        policy.system_event(SystemEvent::ScreenLocked);
        assert_eq!(
            policy.challenge().unwrap_err().code,
            "SYSTEM_SESSION_INACTIVE"
        );
        policy.system_event(SystemEvent::ScreenUnlocked);
        assert!(policy.activate(ticket, 15).is_err());
        assert!(!policy.snapshot().unlocked);
        policy.activate(policy.challenge().unwrap(), 15).unwrap();
        assert!(policy.snapshot().unlocked);
    }

    #[test]
    fn wake_and_session_return_cannot_clear_an_independent_screen_lock() {
        let policy = SessionPolicy::new();
        for event in [
            SystemEvent::ScreenLocked,
            SystemEvent::Sleeping,
            SystemEvent::SessionInactive,
            SystemEvent::DisplaySleeping,
            SystemEvent::Woke,
            SystemEvent::SessionActive,
            SystemEvent::DisplayWoke,
        ] {
            policy.system_event(event);
        }
        assert!(policy.challenge().is_err());
        policy.system_event(SystemEvent::ScreenUnlocked);
        assert!(policy.challenge().is_ok());
        assert!(!policy.snapshot().unlocked);
    }

    #[test]
    fn stale_session_tokens_are_rejected_even_after_reunlock() {
        let policy = SessionPolicy::new();
        let old = policy.activate(0, 15).unwrap();
        policy.revoke("manual");
        let next = policy.activate(policy.challenge().unwrap(), 15).unwrap();
        assert!(policy.require_epoch(old).is_err());
        assert!(policy.require_epoch(next).is_ok());
        assert!(policy.configure(7).is_err());
        policy.configure(0).unwrap();
    }
}
