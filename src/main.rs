mod api;
mod persistency;
mod threads;
mod types;

use std::{
    collections::HashMap,
    fs::canonicalize,
    sync::{Arc, Mutex},
};

use indicatif::ProgressBar;
use persistency::Save;
use types::{DLLRunner, RunnerState, WasmRunner, WasmWorker, WorkerStates};

#[macro_use]
extern crate defer;

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
        bar.set_message("Printing modules");
        for entry in wasm_containers.iter() {
            println!("Wasm module: {}", entry.module_name);
        }

        for entry in wasm_run_containers.iter() {
            println!("Wasm runner: {}", entry.module_name);
        }

        for entry in dll_run_containers.iter() {
            println!("DLL runner: {}", entry.module_name);
        }

        for entry in dll_containers.iter() {
            println!("DLL module: {}", entry.module_name);
        }
    }

    bar.finish_with_message("Finished reading modules");

    let worker_states = Arc::new(Mutex::new(HashMap::new()));
    let native_worker_states = Arc::new(Mutex::new(HashMap::new()));
    let runner_states = Arc::new(Mutex::new(HashMap::new()));
    let native_states = Arc::new(Mutex::new(HashMap::new()));

    threads::spawn_wasm_worker_threads(wasm_containers, worker_states.clone());
    threads::spawn_dll_worker_threads(dll_containers, native_worker_states.clone());
    threads::spawn_wasm_runner_threads(
        wasm_run_containers,
        runner_states.clone(),
        connection_mutex.clone(),
    );
    threads::spawn_dll_runner_threads(
        dll_run_containers,
        native_states.clone(),
        connection_mutex.clone(),
        env_vars_string,
    );

    std::thread::spawn(move || {
        api::create_server(
            worker_states,
            native_worker_states,
            runner_states,
            native_states,
        );
    });

    std::thread::park();
}
