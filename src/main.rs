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
use sentry::{add_breadcrumb, Breadcrumb};
use types::{DLLRunner, GenericPayload, RunnerState, WasmRunner, WasmWorker, WorkerStates};

#[macro_use]
extern crate defer;

fn main() {
    let _guard = sentry::init((
        std::env::var("SENTRY_DSN").unwrap_or_else(|_| "".to_string()),
        sentry::ClientOptions {
            environment: Some(
                std::env::var("RUST_ENV")
                    .unwrap_or_else(|_| "local".to_string())
                    .into(),
            ),
            release: sentry::release_name!(),
            sample_rate: 1.0,
            ..Default::default()
        },
    ));

    let tx_ctx = sentry::TransactionContext::new("startup", "perform-startup");
    let transaction = sentry::start_transaction(tx_ctx);

    // Validate the cart
    let creation_span = transaction.start_child("creation", "Create all the structs");

    let connection = sqlite::open(":memory:").expect("Could not create in memory db");
    let connection_mutex = Arc::new(Mutex::new(connection));
    let bar = ProgressBar::new_spinner();
    let modules_folder_path = match std::env::var("MODULES_PATH") {
        Ok(val) => val,
        Err(err) => {
            sentry::capture_error(&err);
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
        Err(err) => {
            sentry::capture_error(&err);
            panic!("Error: MODULES_PATH folder does not exist");
        }
    };
    bar.set_message("Generating iterator for MODULES_PATH folder");
    let modules_path_iterator = match std::fs::read_dir(modules_folder_path) {
        Ok(val) => val,
        Err(err) => {
            sentry::capture_error(&err);
            panic!("Error: Could not read MODULES_PATH folder - Generating iterator failed");
        }
    };

    let mut wasm_containers: Vec<WasmWorker> = Vec::new();
    let mut wasm_run_containers: Vec<WasmRunner> = Vec::new();
    let mut dll_run_containers: Vec<DLLRunner> = Vec::new();
    let mut dll_containers: Vec<DLLRunner> = Vec::new();

    creation_span.finish();
    bar.set_message("Reading files in MODULES_PATH folder");
    let discovery_span = transaction.start_child("discovery", "Discover all the modules");
    for entry in modules_path_iterator {
        let entry = entry.expect("Error: Could not read entry in MODULES_PATH folder");

        add_breadcrumb(Breadcrumb {
            message: Some(format!(
                "Reading {} file...",
                entry.file_name().to_str().unwrap()
            )),
            ..Default::default()
        });

        let entry_path = entry.path();
        let mut sidecar_json_path = entry_path.clone();
        sidecar_json_path.set_file_name(format!(
            "{}.{}",
            entry.file_name().to_str().unwrap(),
            "json"
        ));

        let sidecar_json_contents =
            std::fs::read(sidecar_json_path).expect("Could not read the sidecar");

        let sidecar: GenericPayload = serde_json::from_str(
            &String::from_utf8(sidecar_json_contents)
                .expect("Could not parse the string from the parts"),
        )
        .expect("Could not parse the sidecar.");

        if entry_path.is_dir() == true
            && entry_path.file_name().unwrap().to_str().unwrap() == "lost+found"
        {
            continue;
        };

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
                stats: sidecar,
            });
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Found runner module: {}",
                    entry.file_name().to_str().unwrap()
                )),
                ..Default::default()
            });
        } else if entry.file_name().to_str().unwrap().ends_with(".wasm") {
            wasm_containers.push(WasmWorker {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                bytes: std::fs::read(entry_path)
                    .expect("Error: Could not read file in MODULES_PATH folder"),
                stats: sidecar,
            });
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Found worker module: {}",
                    entry.file_name().to_str().unwrap()
                )),
                ..Default::default()
            });
        } else if entry.file_name().to_str().unwrap().ends_with("_run.so") {
            dll_run_containers.push(DLLRunner {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                path: canonicalize(entry_path).unwrap().display().to_string(),
                stats: sidecar,
            });
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Found runner DLL: {}",
                    entry.file_name().to_str().unwrap()
                )),
                ..Default::default()
            });
        } else if entry.file_name().to_str().unwrap().ends_with(".so") {
            dll_containers.push(DLLRunner {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                path: canonicalize(entry_path).unwrap().display().to_string(),
                stats: sidecar,
            });
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Found worker DLL: {}",
                    entry.file_name().to_str().unwrap()
                )),
                ..Default::default()
            });
        }
    }
    discovery_span.finish();

    if wasm_containers.is_empty()
        && wasm_run_containers.is_empty()
        && dll_run_containers.is_empty()
        && dll_containers.is_empty()
    {
        bar.finish_with_message("No modules found, exiting...");
        sentry::capture_message("No modules found", sentry::Level::Fatal);
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
    let native_runner_states = Arc::new(Mutex::new(HashMap::new()));

    let threads_span = transaction.start_child("threads", "Creation of threads");

    threads::spawn_wasm_worker_threads(wasm_containers, worker_states.clone());
    threads::spawn_dll_worker_threads(dll_containers, native_worker_states.clone());
    threads::spawn_wasm_runner_threads(
        wasm_run_containers,
        runner_states.clone(),
        connection_mutex.clone(),
    );
    threads::spawn_dll_runner_threads(
        dll_run_containers,
        native_runner_states.clone(),
        connection_mutex.clone(),
        env_vars_string,
    );

    std::thread::spawn(move || {
        api::create_server(
            worker_states,
            native_worker_states,
            runner_states,
            native_runner_states,
        );
    });

    threads_span.finish();

    sentry::capture_message("Health-Check Agent started correctly", sentry::Level::Info);
    transaction.finish();

    std::thread::park();
}
