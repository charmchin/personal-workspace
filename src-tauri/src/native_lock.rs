//! Native observers live on the main thread and never block on the database mutex.
use crate::{database::AppState, session::SystemEvent};
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceScreensDidSleepNotification,
    NSWorkspaceScreensDidWakeNotification, NSWorkspaceSessionDidBecomeActiveNotification,
    NSWorkspaceSessionDidResignActiveNotification, NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSObjectProtocol,
    NSString,
};
use std::{cell::RefCell, ptr::NonNull};
use tauri::{Emitter, Manager};

pub(crate) const LOCK_EVENT: &str = "workbench:locked";
const FOCUS_EVENT: &str = "com.local.personalworkbench.focus-existing-window";

pub(crate) fn request_focus() {
    unsafe {
        NSDistributedNotificationCenter::defaultCenter()
            .postNotificationName_object_userInfo_deliverImmediately(
                &NSString::from_str(FOCUS_EVENT),
                None,
                None,
                true,
            );
    }
}

pub(crate) struct Observer {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Observer {
    fn drop(&mut self) {
        // The token belongs to this exact center; both retained objects are alive here.
        unsafe {
            self.center.removeObserver((*self.token).as_ref());
        }
    }
}

thread_local! { static OBSERVERS: RefCell<Vec<Observer>> = const { RefCell::new(Vec::new()) }; }

pub(crate) fn observe(
    center: Retained<NSNotificationCenter>,
    name: &NSString,
    callback: impl Fn() + Send + Sync + 'static,
) -> Observer {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| callback());
    // A nil sender matches all system senders. The callback captures only Send + Sync values;
    // queue=nil executes on the posting thread, with no database or UI-thread blocking.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    Observer { center, token }
}

pub(crate) fn install(app: &tauri::AppHandle) -> Result<(), &'static str> {
    let _main_thread = MainThreadMarker::new().ok_or("原生锁定监听必须在主线程启动")?;
    let handle = app.clone();
    let mut observers = register_notifications(move |event| system_event(&handle, event));
    let handle = app.clone();
    observers.push(observe(
        NSDistributedNotificationCenter::defaultCenter().into_super(),
        &NSString::from_str(FOCUS_EVENT),
        move || {
            // This untrusted notification can only show/focus a window, never unlock data.
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        },
    ));
    OBSERVERS.with(|slot| *slot.borrow_mut() = observers);
    Ok(())
}

pub(crate) fn register_notifications(
    callback: impl Fn(SystemEvent) + Clone + Send + Sync + 'static,
) -> Vec<Observer> {
    let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
    let distributed: Retained<NSNotificationCenter> =
        NSDistributedNotificationCenter::defaultCenter().into_super();
    let mut observers = Vec::new();
    // These distributed names are system conventions, not documented lock-screen APIs.
    // They only revoke access: untrusted/spoofed notifications cannot unlock the database.
    for (name, event) in [
        ("com.apple.screenIsLocked", SystemEvent::ScreenLocked),
        ("com.apple.screenIsUnlocked", SystemEvent::ScreenUnlocked),
    ] {
        let handler = callback.clone();
        observers.push(observe(
            distributed.clone(),
            &NSString::from_str(name),
            move || handler(event),
        ));
    }
    // Public workspace notifications also protect sleep, display sleep, and user switching.
    let names = unsafe {
        [
            (NSWorkspaceWillSleepNotification, SystemEvent::Sleeping),
            (NSWorkspaceDidWakeNotification, SystemEvent::Woke),
            (
                NSWorkspaceSessionDidResignActiveNotification,
                SystemEvent::SessionInactive,
            ),
            (
                NSWorkspaceSessionDidBecomeActiveNotification,
                SystemEvent::SessionActive,
            ),
            (
                NSWorkspaceScreensDidSleepNotification,
                SystemEvent::DisplaySleeping,
            ),
            (
                NSWorkspaceScreensDidWakeNotification,
                SystemEvent::DisplayWoke,
            ),
        ]
    };
    for (name, event) in names {
        let handler = callback.clone();
        observers.push(observe(workspace.clone(), name, move || handler(event)));
    }
    observers
}

fn system_event(app: &tauri::AppHandle, event: SystemEvent) {
    let state = app.state::<AppState>();
    let snapshot = state.session.system_event(event);
    let _ = app.emit_to("main", LOCK_EVENT, snapshot);
}

pub(crate) fn remove() {
    OBSERVERS.with(|slot| slot.borrow_mut().clear());
}

pub(crate) fn start_monitor(app: &tauri::AppHandle) -> std::thread::JoinHandle<()> {
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut last_epoch = 0;
        while !handle
            .state::<AppState>()
            .stopping
            .load(std::sync::atomic::Ordering::Acquire)
        {
            let state = handle.state::<AppState>();
            let snapshot = state.session.snapshot();
            if !snapshot.unlocked {
                if snapshot.session_epoch != last_epoch {
                    let _ = handle.emit_to("main", LOCK_EVENT, &snapshot);
                }
                state.close_revoked_session();
            }
            last_epoch = snapshot.session_epoch;
            std::thread::park_timeout(std::time::Duration::from_millis(250));
        }
    })
}

pub(crate) fn start_snapshot_monitor(app: &tauri::AppHandle) -> std::thread::JoinHandle<()> {
    let handle = app.clone();
    std::thread::spawn(move || {
        loop {
            let state = handle.state::<AppState>();
            if state.stopping.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            let _ = state.refresh_snapshot_if_due(false);
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
    })
}
