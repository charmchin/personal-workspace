//! Main-thread native smoke test. No real lock/sleep signals or user data are used.
#![allow(dead_code)]
#[path = "../src/database.rs"]
mod database;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/native_lock.rs"]
mod native_lock;
#[path = "../src/restore.rs"]
mod restore;
#[allow(unused_imports)] // The harness-free executable does not run this module's unit tests.
#[path = "../src/session.rs"]
mod session;

use objc2::MainThreadMarker;
use objc2_foundation::{
    NSDate, NSDistributedNotificationCenter, NSNotificationCenter, NSRunLoop, NSString,
};
use session::{SessionPolicy, SystemEvent};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn main() {
    objc2::rc::autoreleasepool(|_| {
        assert!(MainThreadMarker::new().is_some());
        // Register real subscriptions, but never broadcast names that would affect other apps.
        let subscriptions = native_lock::register_notifications(|_| {});
        assert_eq!(subscriptions.len(), 8);
        let policy = Arc::new(SessionPolicy::new());
        policy.activate(0, 0).unwrap();
        let name = NSString::from_str(&format!(
            "com.local.personalworkbench.test.{}",
            std::process::id()
        ));
        let local = NSNotificationCenter::new();
        let callback = policy.clone();
        let observer = native_lock::observe(local.clone(), &name, move || {
            callback.system_event(SystemEvent::ScreenLocked);
        });
        unsafe {
            local.postNotificationName_object(&name, None);
        }
        assert!(!policy.snapshot().unlocked);
        drop(observer);
        policy.system_event(SystemEvent::ScreenUnlocked);
        policy.activate(policy.challenge().unwrap(), 0).unwrap();
        unsafe {
            local.postNotificationName_object(&name, None);
        }
        assert!(policy.snapshot().unlocked, "observer must be removed");

        let distributed = NSDistributedNotificationCenter::defaultCenter();
        let callback = policy.clone();
        let observer = native_lock::observe(distributed.clone().into_super(), &name, move || {
            callback.system_event(SystemEvent::ScreenLocked);
        });
        unsafe {
            distributed
                .postNotificationName_object_userInfo_deliverImmediately(&name, None, None, true);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while policy.snapshot().unlocked && Instant::now() < deadline {
            NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.01));
        }
        assert!(
            !policy.snapshot().unlocked,
            "distributed callback was not delivered"
        );
        drop(observer);
        drop(subscriptions);
        println!(
            "native_lock_smoke: 8 subscriptions, local/distributed callbacks, authorization revocation and observer cleanup passed; no real system lock triggered"
        );
    });
}
