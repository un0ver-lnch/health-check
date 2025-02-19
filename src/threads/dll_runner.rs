use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
};

use libloading::{Library, Symbol};
use sentry::{add_breadcrumb, Breadcrumb, Level};
use sqlite::Connection;

use crate::{
    persistency::{self, Save},
    types,
};

pub fn spawn_dll_runner_threads(
    dll_run_containers: Vec<types::DLLRunner>,
    native_states: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, types::NativeStates>>,
    >,
    native_connection: std::sync::Arc<std::sync::Mutex<Connection>>,
    env_vars_string: String,
) {
    for native_runner in dll_run_containers {
        add_breadcrumb(Breadcrumb {
            message: Some(format!(
                "Initializing DLL runner for: {}",
                native_runner.module_name
            )),
            level: Level::Info,
            ..Default::default()
        });

        let native_states = native_states.clone();
        let native_connection = native_connection.clone();
        let env_vars_string = env_vars_string.clone();

        let (channel_trigger, channel_reciver) = std::sync::mpsc::channel::<()>();

        let lib = unsafe { Library::new(&native_runner.path) };

        if let Err(err) = lib {
            add_breadcrumb(Breadcrumb {
                message: Some(format!("Failed to load DLL: {}", native_runner.module_name)),
                level: Level::Error,
                ..Default::default()
            });
            sentry::capture_error(&err);
            eprintln!("Error: Could not load library: {}", err);
            native_states.lock().unwrap().insert(
                native_runner.module_name.clone(),
                types::NativeStates {
                    module_name: native_runner.module_name.clone(),
                    on_crash: true,
                    last_run: std::time::Instant::now(),
                    last_run_success: false,
                    channel_trigger,
                },
            );
            continue;
        }
        let lib = lib.unwrap();

        native_states.lock().unwrap().insert(
            native_runner.module_name.clone(),
            types::NativeStates {
                module_name: native_runner.module_name.clone(),
                on_crash: false,
                last_run: std::time::Instant::now(),
                last_run_success: false,
                channel_trigger,
            },
        );

        std::thread::spawn(move || {
            let exec_lib_func: Symbol<unsafe extern "C" fn(env: *const c_char) -> *const c_char> =
                unsafe { lib.get(b"start").unwrap() };
            let exec_lib_result_free: Symbol<unsafe extern "C" fn(*const c_char) -> ()> =
                unsafe { lib.get(b"free_string").unwrap() };

            while let Ok(_) = channel_reciver.recv() {
                process_lib_execution(
                    &native_runner,
                    &native_states,
                    &native_connection,
                    &env_vars_string,
                    &exec_lib_func,
                    &exec_lib_result_free,
                );
            }
        });
    }
}

fn process_lib_execution(
    native_runner: &types::DLLRunner,
    native_states: &std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, types::NativeStates>>,
    >,
    native_connection: &std::sync::Arc<std::sync::Mutex<Connection>>,
    env_vars_string: &str,
    exec_lib_func: &Symbol<unsafe extern "C" fn(*const c_char) -> *const c_char>,
    exec_lib_result_free: &Symbol<unsafe extern "C" fn(*const c_char) -> ()>,
) {
    add_breadcrumb(Breadcrumb {
        message: Some(format!(
            "Starting DLL execution for: {}",
            native_runner.module_name
        )),
        level: Level::Info,
        ..Default::default()
    });

    let env_vars_string = match CString::new(env_vars_string.to_string()) {
        Ok(val) => val,
        Err(err) => {
            add_breadcrumb(Breadcrumb {
                message: Some("Failed to create CString for env vars".into()),
                level: Level::Error,
                ..Default::default()
            });
            sentry::capture_error(&err);
            return;
        }
    };

    let raw = env_vars_string.into_raw();
    let result = unsafe { exec_lib_func(raw) };

    defer! {
        unsafe { exec_lib_result_free(result) }
    };

    unsafe {
        let _env_vars_string = CString::from_raw(raw);
    };

    let result_as_string = unsafe { CStr::from_ptr(result as *mut c_char) }
        .to_string_lossy()
        .to_string();

    for out_line in result_as_string.split('\n') {
        if !out_line.starts_with("KV:") {
            continue;
        }

        let filtered_data = out_line.replace("KV:", "");
        let key_value_split: Vec<&str> = filtered_data.split("###").collect();

        if key_value_split.len() != 2 {
            add_breadcrumb(Breadcrumb {
                message: Some(format!("Invalid KV format in DLL output: {}", out_line)),
                level: Level::Warning,
                ..Default::default()
            });
            continue;
        }

        let key_value_pair = persistency::KeyValuePair {
            key: key_value_split[0].to_string(),
            value: key_value_split[1].to_string(),
        };

        if let Ok(_) = key_value_pair.persist(&native_connection.lock().unwrap()) {
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "DLL {} persisted KV pair - Key: {}",
                    native_runner.module_name, key_value_pair.key
                )),
                level: Level::Debug,
                ..Default::default()
            });
        } else {
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Failed to persist KV pair from DLL {} - Key: {}",
                    native_runner.module_name, key_value_pair.key
                )),
                level: Level::Error,
                ..Default::default()
            });
        }
    }

    if let Ok(mut native_state_lock) = native_states.lock() {
        if let Some(native_state) = native_state_lock.get_mut(&native_runner.module_name) {
            native_state.last_run_success = true;
            native_state.last_run = std::time::Instant::now();
        }
    }
}
