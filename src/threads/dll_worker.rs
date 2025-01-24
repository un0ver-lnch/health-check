use std::{
    collections::HashMap,
    ffi::CStr,
    os::raw::c_char,
    sync::{Arc, Mutex},
};

use libloading::{Library, Symbol};

use crate::types::{self, DLLRunner, NativeWorkerStates};

pub fn spawn_dll_worker_threads(
    dll_containers: Vec<DLLRunner>,
    native_worker_states: Arc<Mutex<HashMap<String, NativeWorkerStates>>>,
) {
    for entry in dll_containers {
        let native_worker_states = native_worker_states.clone();
        native_worker_states.lock().unwrap().insert(
            entry.module_name.clone(),
            types::NativeWorkerStates {
                alive: false,
                on_crash: false,
            },
        );

        let lib = unsafe { Library::new(&entry.path) };

        if let Err(val) = lib {
            eprintln!("Error: Could not load library: {}", val);
            native_worker_states.lock().unwrap().insert(
                entry.module_name,
                types::NativeWorkerStates {
                    alive: false,
                    on_crash: true,
                },
            );
            continue;
        }
        let lib = lib.unwrap();

        std::thread::spawn(move || run_dll_worker(entry, native_worker_states, lib));
    }
}

fn run_dll_worker(
    entry: DLLRunner,
    native_worker_states: Arc<Mutex<HashMap<String, NativeWorkerStates>>>,
    lib: Library,
) {
    let exec_lib_func: Symbol<unsafe extern "C" fn() -> *const c_char> =
        unsafe { lib.get(b"start").unwrap() };
    let exec_lib_result_free: Symbol<unsafe extern "C" fn(*const c_char) -> ()> =
        unsafe { lib.get(b"free_string").unwrap() };

    loop {
        let result = unsafe { exec_lib_func() };
        defer! {
            unsafe { exec_lib_result_free(result) }
        };

        let result_as_string = unsafe { CStr::from_ptr(result as *mut c_char) }
            .to_string_lossy()
            .to_string();

        let status = match result_as_string.lines().next().unwrap_or("") {
            "True" => Some((true, false)),
            "False" => Some((false, false)),
            "Crash" => Some((false, true)),
            _ => None,
        };

        if let Some((alive, on_crash)) = status {
            if let Ok(mut state_lock) = native_worker_states.lock() {
                if let Some(state) = state_lock.get_mut(&entry.module_name) {
                    state.alive = alive;
                    state.on_crash = on_crash;
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
