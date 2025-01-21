mod api;

mod types;

mod persistency;

use libloading::{Library, Symbol};
use persistency::Save;

use types::{DLLRunner, RunnerState, WasmRunner, WasmWorker, WorkerStates};

use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    fs::canonicalize,
    io::Read,
    os::raw::c_char,
    sync::{Arc, Mutex},
};

use indicatif::ProgressBar;
use wasmer::{Module, Store};
#[macro_use]
extern crate defer;

use wasmer_wasix::{Pipe, WasiEnv};

fn main() {
    let connection = sqlite::open(":memory:").expect("Could not create in memory db");
    let connection_mutex = Arc::new(Mutex::new(connection));
    let bar = ProgressBar::new_spinner();
    let modules_folder_path = match std::env::var("MODULES_PATH") {
        Ok(val) => val,
        Err(_) => {
            panic!("Error: MODULES_PATH env variable not set");
        }
    };
    bar.set_message("Generate full environment variables string");

    let mut env_vars = std::env::vars();
    let mut env_vars_string = String::new();
    while let Some((key, value)) = env_vars.next() {
        env_vars_string.push_str(&format!("{}={};;;", key, value));
    }

    bar.set_message("Checking if MODULES_PATH folder exists");
    match std::fs::exists(&modules_folder_path) {
        Ok(val) => {
            if val == false {
                panic!("Error: MODULES_PATH folder does not exist");
            }
        }
        Err(_) => {
            panic!("Error: MODULES_PATH folder does not exist");
        }
    };
    bar.set_message("Generating iterator for MODULES_PATH folder");
    let modules_path_iterator = match std::fs::read_dir(modules_folder_path) {
        Ok(val) => val,
        Err(_) => {
            panic!("Error: Could not read MODULES_PATH folder - Generating iterator failed");
        }
    };

    let mut wasm_containers: Vec<WasmWorker> = Vec::new();
    let mut wasm_run_containers: Vec<WasmRunner> = Vec::new();
    let mut dll_run_containers: Vec<DLLRunner> = Vec::new();
    let mut dll_containers: Vec<DLLRunner> = Vec::new();
    bar.set_message("Reading files in MODULES_PATH folder");
    for entry in modules_path_iterator {
        let entry = entry.expect("Error: Could not read entry in MODULES_PATH folder");

        let entry_path = entry.path();

        if entry_path.is_dir() == true
            && entry_path.file_name().unwrap().to_str().unwrap() == "lost+found"
        {
            continue;
        }

        if entry_path.is_dir() {
            panic!("Error: MODULES_PATH folder contains a directory");
        }

        bar.set_message(format!(
            "Reading {} file...",
            entry.file_name().to_str().unwrap()
        ));
        if entry.file_name().to_str().unwrap().ends_with("_run.wasm") {
            wasm_run_containers.push(WasmRunner {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                bytes: std::fs::read(entry_path)
                    .expect("Error: Could not read file in MODULES_PATH folder"),
            });
        } else if entry.file_name().to_str().unwrap().ends_with(".wasm") {
            wasm_containers.push(WasmWorker {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                bytes: std::fs::read(entry_path)
                    .expect("Error: Could not read file in MODULES_PATH folder"),
            });
        } else if entry.file_name().to_str().unwrap().ends_with("_run.so") {
            dll_run_containers.push(DLLRunner {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                path: canonicalize(entry_path).unwrap().display().to_string(),
            });
        } else if entry.file_name().to_str().unwrap().ends_with(".so") {
            dll_containers.push(DLLRunner {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                path: canonicalize(entry_path).unwrap().display().to_string(),
            });
        }
    }

    if wasm_containers.is_empty()
        && wasm_run_containers.is_empty()
        && dll_run_containers.is_empty()
        && dll_containers.is_empty()
    {
        bar.finish_with_message("No modules found, exiting...");
        return;
    }

    let modules_folder_path = match std::env::var("SHOW_MODULES_CONSOLE") {
        Ok(_) => true,
        Err(_) => false,
    };

    if modules_folder_path {
        bar.set_message("Reading modules");
        for entry in wasm_containers.iter() {
            bar.println(format!("Wasm module: {}", entry.module_name));
        }

        for entry in wasm_run_containers.iter() {
            bar.println(format!("Wasm runner: {}", entry.module_name));
        }

        for entry in dll_run_containers.iter() {
            bar.println(format!("DLL runner: {}", entry.module_name));
        }

        for entry in dll_containers.iter() {
            bar.println(format!("DLL module: {}", entry.module_name));
        }
    }

    bar.finish_with_message("Finished reading modules");

    let worker_states = Arc::new(Mutex::new(HashMap::new()));
    let worker_connection = connection_mutex.clone();

    for entry in wasm_containers {
        let worker_states = worker_states.clone();
        worker_states.lock().unwrap().insert(
            entry.module_name.clone(),
            WorkerStates {
                alive: false,
                on_crash: false,
            },
        );
        std::thread::spawn(move || {
            // Create a Store.
            let mut store = Store::default();
            // Let's compile the Wasm module.
            let module = match Module::new(&store, &entry.bytes) {
                Ok(val) => val,
                Err(_) => {
                    panic!("Error: Could not compile Wasm module");
                }
            };

            // let (stdout_tx, stdout_rx) = Pipe::channel();
            // let stdout_rx = Box::new(stdout_rx);
            // let stdout_tx = Box::new(stdout_tx);
            let module = Box::new(module);
            loop {
                let (stdout_tx, mut stdout_rx) = Pipe::channel();
                //let mut stdout_rx = stdout_rx.clone();
                //let stdout_tx = stdout_tx.clone();
                let module = module.clone();

                // Run the module.
                let builder = WasiEnv::builder(&entry.module_name)
                    // .args(&["world"])
                    // .env("KEY", "Value")
                    .stdout(Box::new(stdout_tx))
                    .run_with_store(*module, &mut store);

                let _ = match builder {
                    Ok(val) => val,
                    Err(_) => {
                        worker_states.lock().unwrap().insert(
                            (&entry.module_name).to_string(),
                            WorkerStates {
                                alive: false,
                                on_crash: true,
                            },
                        );

                        panic!("Error: Could not run WasiEnv builder");
                    }
                };

                // FIXME: Add better implementation of a health check.

                let mut buf = String::new();
                stdout_rx.read_to_string(&mut buf).unwrap();

                // println!("Read \"{}\" from the WASI stdout!", buf.trim());
                // println!("{} == {} = {}", buf, "true", buf.trim().eq("true"));

                if buf.eq("true") {
                    worker_states.lock().unwrap().insert(
                        (&entry.module_name).to_string(),
                        WorkerStates {
                            alive: true,
                            on_crash: false,
                        },
                    );
                } else {
                    worker_states.lock().unwrap().insert(
                        (&entry.module_name).to_string(),
                        WorkerStates {
                            alive: false,
                            on_crash: false,
                        },
                    );
                }

                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });
    }

    let native_worker_states = Arc::new(Mutex::new(HashMap::new()));
    let native_worker_connection = connection_mutex.clone();

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

        let lib = match lib {
            Ok(val) => val,
            Err(val) => {
                eprintln!("Error: Could not load library: {}", val);
                native_worker_states.lock().unwrap().insert(
                    entry.module_name.clone(),
                    types::NativeWorkerStates {
                        alive: false,
                        on_crash: true,
                    },
                );
                continue;
            }
        };

        std::thread::spawn(move || {
            let native_worker_states = native_worker_states.clone();

            let exec_lib_func: Symbol<unsafe extern "C" fn() -> *const c_char> =
                unsafe { lib.get(b"start").unwrap() };
            let exec_lib_result_free: Symbol<unsafe extern "C" fn(*const c_char) -> ()> =
                unsafe { lib.get(b"free_string").unwrap() };

            loop {
                let result = unsafe { exec_lib_func() };
                defer! {
                    // We need to free the result string
                    unsafe { exec_lib_result_free(result) }
                };
                let result_as_string = unsafe { CStr::from_ptr(result as *mut c_char) };

                let result_as_string = result_as_string.to_string_lossy().to_string();

                let result_splited = result_as_string.split("\n").collect::<Vec<&str>>();

                assert!(result_splited.len() == 1);

                for out_line in result_splited {
                    match out_line {
                        "True" => {
                            let mut native_state_lock = native_worker_states.lock().unwrap();

                            let native_state =
                                native_state_lock.get_mut(&entry.module_name).unwrap();

                            native_state.alive = true;
                        }
                        "False" => {
                            let mut native_state_lock = native_worker_states.lock().unwrap();

                            let native_state =
                                native_state_lock.get_mut(&entry.module_name).unwrap();

                            native_state.alive = false;
                        }

                        "Crash" => {
                            let mut native_state_lock = native_worker_states.lock().unwrap();

                            let native_state =
                                native_state_lock.get_mut(&entry.module_name).unwrap();

                            native_state.alive = false;
                            native_state.on_crash = true;
                        }
                        _ => {
                            panic!("Error: Unknown response from native worker");
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });
    }

    let runner_states = Arc::new(Mutex::new(HashMap::new()));
    let runner_connection = connection_mutex.clone();

    for runner in wasm_run_containers {
        let runner_connection = runner_connection.clone();
        let runner_states = runner_states.clone();

        let (channel_trigger, channel_reciver) = std::sync::mpsc::channel();

        runner_states.lock().unwrap().insert(
            runner.module_name.clone(),
            RunnerState {
                module_name: runner.module_name.clone(),
                last_run: std::time::Instant::now(),
                last_run_success: false,
                channel_trigger,
            },
        );

        std::thread::spawn(move || {
            // Create a Store.
            let mut store = Store::default();
            // Let's compile the Wasm module.
            let module = match Module::new(&store, &runner.bytes) {
                Ok(val) => val,
                Err(_) => {
                    panic!("Error: Could not compile Wasm module");
                }
            };

            // let (stdout_tx, stdout_rx) = Pipe::channel();
            // let stdout_rx = Box::new(stdout_rx);
            // let stdout_tx = Box::new(stdout_tx);
            let module = Box::new(module);
            while let Ok(_) = channel_reciver.recv() {
                let (stdout_tx, mut stdout_rx) = Pipe::channel();
                let (stderr_tx, mut stderr_rx) = Pipe::channel();
                //let mut stdout_rx = stdout_rx.clone();
                //let stdout_tx = stdout_tx.clone();
                let module = module.clone();

                // Run the module.
                let builder = WasiEnv::builder(&runner.module_name)
                    // .args(&["world"])
                    // .env("KEY", "Value")
                    .stdout(Box::new(stdout_tx))
                    .stderr(Box::new(stderr_tx))
                    .run_with_store(*module, &mut store);

                let _ = match builder {
                    Ok(val) => val,
                    Err(_) => {
                        runner_states
                            .lock()
                            .unwrap()
                            .get_mut(&runner.module_name)
                            .unwrap()
                            .last_run_success = false;
                        panic!("Error: Could not run WasiEnv builder");
                    }
                };

                // FIXME: Add better implementation of a health check.

                let mut buf = String::new();
                stdout_rx.read_to_string(&mut buf).unwrap();

                let stdout_split = buf.split("\n").collect::<Vec<&str>>();

                for out_line in stdout_split {
                    if out_line.starts_with("KV:") {
                        let filtered_data = out_line.replace("KV:", "");

                        let key_value_split = filtered_data.split("###").collect::<Vec<&str>>();

                        let key = key_value_split[0].to_string();
                        let value = key_value_split[1].to_string();

                        let key_value_pair = persistency::KeyValuePair { key, value };

                        match key_value_pair.persist(&runner_connection.lock().unwrap()) {
                            Ok(_) => {
                                println!(
                                    "Persisted key: {} with value: {}",
                                    &key_value_pair.key, &key_value_pair.value
                                );
                            }
                            Err(_) => {
                                eprintln!(
                                    "Error: Could not persist key: {} with value: {}",
                                    &key_value_pair.key, &key_value_pair.value
                                );
                            }
                        };
                    }
                }

                let mut bur_err = String::new();
                stderr_rx.read_to_string(&mut bur_err).unwrap();

                let stderr_split = bur_err.split("\n").collect::<Vec<&str>>();

                for err_line in stderr_split {
                    println!("RUNNER {}: {}", &runner.module_name, err_line);
                }

                // println!("Read \"{}\" from the WASI stdout!", buf.trim());
                // println!("{} == {} = {}", buf, "true", buf.trim().eq("true"));
            }
        });
    }

    let native_states = Arc::new(Mutex::new(HashMap::new()));
    let native_connection = connection_mutex.clone();
    for native_runner in dll_run_containers {
        let native_states = native_states.clone();
        let native_connection = native_connection.clone();
        let env_vars_string = env_vars_string.clone();

        let lib = unsafe { Library::new(&native_runner.path) };

        let (channel_trigger, channel_reciver) = std::sync::mpsc::channel::<()>();

        let lib = match lib {
            Ok(val) => val,
            Err(val) => {
                eprintln!("Error: Could not load library: {}", val);
                native_states.lock().unwrap().insert(
                    native_runner.module_name.clone(),
                    types::NativeStates {
                        on_crash: true,
                        last_run: std::time::Instant::now(),
                        last_run_success: false,
                        channel_trigger,
                    },
                );
                continue;
            }
        };

        native_states.lock().unwrap().insert(
            native_runner.module_name.clone(),
            types::NativeStates {
                on_crash: false,
                last_run: std::time::Instant::now(),
                last_run_success: false,
                channel_trigger,
            },
        );

        std::thread::spawn(move || {
            let native_states = native_states.clone();
            let env_vars_string = env_vars_string.clone();

            let exec_lib_func: Symbol<unsafe extern "C" fn(env: *const c_char) -> *const c_char> =
                unsafe { lib.get(b"start").unwrap() };
            let exec_lib_result_free: Symbol<unsafe extern "C" fn(*const c_char) -> ()> =
                unsafe { lib.get(b"free_string").unwrap() };

            while let Ok(_) = channel_reciver.recv() {
                let env_vars_string = CString::new(env_vars_string.clone()).unwrap();
                let raw = env_vars_string.into_raw();
                let result = unsafe { exec_lib_func(raw) };
                defer! {
                    // We need to free the result string
                    unsafe { exec_lib_result_free(result) }
                };
                unsafe {
                    let _env_vars_string = CString::from_raw(raw);
                };
                let result_as_string = unsafe { CStr::from_ptr(result as *mut c_char) };

                let result_as_string = result_as_string.to_string_lossy().to_string();

                let result_splited = result_as_string.split("\n").collect::<Vec<&str>>();

                for out_line in result_splited {
                    if out_line.starts_with("KV:") {
                        let filtered_data = out_line.replace("KV:", "");

                        let key_value_split = filtered_data.split("###").collect::<Vec<&str>>();

                        let key = key_value_split[0].to_string();
                        let value = key_value_split[1].to_string();

                        let key_value_pair = persistency::KeyValuePair { key, value };

                        match key_value_pair.persist(&native_connection.lock().unwrap()) {
                            Ok(_) => {
                                println!(
                                    "Persisted key: {} with value: {}",
                                    &key_value_pair.key, &key_value_pair.value
                                );
                            }
                            Err(_) => {
                                eprintln!(
                                    "Error: Could not persist key: {} with value: {}",
                                    &key_value_pair.key, &key_value_pair.value
                                );
                            }
                        };
                    }
                }
                let mut native_state_lock = native_states.lock().unwrap();

                let native_state = native_state_lock
                    .get_mut(&native_runner.module_name)
                    .unwrap();

                native_state.last_run_success = true;
                native_state.last_run = std::time::Instant::now();
            }
        });
    }

    std::thread::spawn(move || {
        //let worker_states = worker_states.clone();
        api::create_server(
            worker_states,
            native_worker_states,
            runner_states,
            native_states,
        );
    });

    std::thread::park();
}
