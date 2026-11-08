//! Resource limiter: RAM monitoring, tab sleeping, Game Mode, process priority.

use crate::{emit_state, Browser};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Clone)]
struct Stats {
    mem_mb: u64,
    cap_mb: u64,
    tabs: usize,
    sleeping: usize,
    game_mode: bool,
    blocked_domains: usize,
}

/// Our process plus every descendant (WebView2 browser, renderer, GPU processes...).
fn process_tree(sys: &System) -> Vec<Pid> {
    let Ok(me) = sysinfo::get_current_pid() else { return vec![] };
    let mut out = vec![me];
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        for (pid, p) in sys.processes() {
            if p.parent() == Some(parent) && !out.contains(pid) {
                out.push(*pid);
            }
        }
        i += 1;
    }
    out
}

#[cfg(windows)]
fn set_priority(pids: &[Pid], low: bool) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        PROCESS_SET_INFORMATION,
    };
    let class = if low { BELOW_NORMAL_PRIORITY_CLASS } else { NORMAL_PRIORITY_CLASS };
    for pid in pids {
        unsafe {
            let h = OpenProcess(PROCESS_SET_INFORMATION, 0, pid.as_u32());
            if !h.is_null() {
                SetPriorityClass(h, class);
                CloseHandle(h);
            }
        }
    }
}

#[cfg(not(windows))]
fn set_priority(_pids: &[Pid], _low: bool) {}

/// Puts tabs to sleep: closes their webview (freeing its renderer memory) but
/// keeps URL + title so the tab reloads when clicked.
pub fn discard(app: &AppHandle, ids: &[u32]) {
    for id in ids {
        if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
            let _ = wv.close();
        }
    }
    if !ids.is_empty() {
        emit_state(app);
    }
}

pub fn set_game_mode(app: &AppHandle, on: bool) {
    let victims: Vec<u32> = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.game_mode = on;
        let active = b.active;
        if on && b.settings.auto_unload {
            b.tabs
                .iter_mut()
                .filter(|t| Some(t.id) != active && !t.discarded && !t.pinned)
                .map(|t| {
                    t.discarded = true;
                    t.id
                })
                .collect()
        } else {
            vec![]
        }
    };
    discard(app, &victims);
    std::thread::spawn(move || {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        set_priority(&process_tree(&sys), on);
    });
    let _ = app.emit("toast", if on { "Game Mode ON — browser priority lowered" } else { "Game Mode OFF" });
    emit_state(app);
}

pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut sys = System::new();
        loop {
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let pids = process_tree(&sys);
            let mem_mb: u64 =
                pids.iter().filter_map(|p| sys.process(*p)).map(|p| p.memory()).sum::<u64>() / 1_048_576;

            let (victims, cap_mb, tabs, sleeping, game_mode) = {
                let st = app.state::<Mutex<Browser>>();
                let mut b = st.lock().unwrap();
                let game_mode = b.game_mode;
                let cap = if game_mode { b.settings.game_mem_cap_mb } else { b.settings.mem_cap_mb };
                let auto = b.settings.auto_unload;
                let idle = if auto { b.settings.discard_idle_min } else { 0 };
                let active = b.active;
                let mut victims = vec![];

                // 1. Idle background tabs.
                if idle > 0 {
                    for t in b.tabs.iter_mut() {
                        if Some(t.id) != active
                            && !t.discarded
                            && !t.pinned
                            && t.last_active.elapsed().as_secs() > idle * 60
                        {
                            t.discarded = true;
                            victims.push(t.id);
                        }
                    }
                }
                // 2. Over the RAM cap: sleep the least-recently-used background tab.
                if auto && cap > 0 && mem_mb > cap {
                    if let Some(t) = b
                        .tabs
                        .iter_mut()
                        .filter(|t| Some(t.id) != active && !t.discarded && !t.pinned)
                        .min_by_key(|t| t.last_active)
                    {
                        t.discarded = true;
                        victims.push(t.id);
                    }
                }
                let sleeping = b.tabs.iter().filter(|t| t.discarded).count();
                (victims, cap, b.tabs.len(), sleeping, game_mode)
            };

            discard(&app, &victims);
            if game_mode {
                // New renderer processes appear as you browse; keep them deprioritized.
                set_priority(&pids, true);
            }
            let _ = app.emit(
                "stats",
                Stats { mem_mb, cap_mb, tabs, sleeping, game_mode, blocked_domains: crate::adblock::blocked_count() },
            );
            crate::history::flush(&app);
            crate::session::save(&app);
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}
