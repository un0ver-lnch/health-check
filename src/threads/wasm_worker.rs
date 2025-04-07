use sentry::{add_breadcrumb, Breadcrumb, Level};
use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
};
use wasmer::{Module, Store};
use wasmer_wasix::{Pipe, WasiEnv};

use crate::types::{WasmWorker, WorkerStates};

pub fn spawn_wasm_worker_threads(
    wasm_containers: Vec<WasmWorker>,
    worker_states: Arc<Mutex<HashMap<String, WorkerStates>>>,
) {
    for entry in wasm_containers {
        let worker_states = worker_states.clone();
        worker_states.lock().unwrap().insert(
            entry.module_name.clone(),
            WorkerStates {
                alive: false,
                on_crash: false,
            },
        );

        std::thread::spawn(move || run_wasm_worker(entry, worker_states));
    }
}

fn run_wasm_worker(entry: WasmWorker, worker_states: Arc<Mutex<HashMap<String, WorkerStates>>>) {
    add_breadcrumb(Breadcrumb {
        message: Some(format!(
            "Initializing WASM worker for module: {}",
            entry.module_name
        )),
        level: Level::Info,
        ..Default::default()
    });

    let mut store = Store::default();
    let module = match Module::new(&store, &entry.bytes) {
        Ok(val) => {
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Successfully compiled WASM module: {}",
                    entry.module_name
                )),
                level: Level::Info,
                ..Default::default()
            });
            Box::new(val)
        }
        Err(err) => {
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "Failed to compile WASM module: {}",
                    entry.module_name
                )),
                level: Level::Error,
                ..Default::default()
            });
            sentry::capture_error(&err);
            eprintln!("Error: Could not compile Wasm module: {}", err);
            worker_states.lock().unwrap().insert(
                entry.module_name,
                WorkerStates {
                    alive: false,
                    on_crash: true,
                },
            );
            return;
        }
    };

    let (stdout_tx, mut stdout_rx) = Pipe::channel();
    let builder = WasiEnv::builder(&entry.module_name)
            .stdout(Box::new(stdout_tx))
            .run_with_store(*module, &mut store);

    loop {

        if let Err(err) = builder {
            add_breadcrumb(Breadcrumb {
                message: Some(format!(
                    "WASM module execution failed: {}",
                    entry.module_name
                )),
                level: Level::Error,
                ..Default::default()
            });
            sentry::capture_error(&err);
            worker_states.lock().unwrap().insert(
                entry.module_name.clone(),
                WorkerStates {
                    alive: false,
                    on_crash: true,
                },
            );
            return;
        }

        let mut buf = String::new();
        if let Err(err) = stdout_rx.read_to_string(&mut buf) {
            add_breadcrumb(Breadcrumb {
                message: Some(format!("Failed to read WASM output: {}", entry.module_name)),
                level: Level::Error,
                ..Default::default()
            });
            sentry::capture_error(&err);
            continue;
        }

        let is_alive = buf.eq("true");
        add_breadcrumb(Breadcrumb {
            message: Some(format!(
                "WASM worker status - Module: {}, Alive: {}",
                entry.module_name, is_alive
            )),
            level: if is_alive {
                Level::Info
            } else {
                Level::Warning
            },
            ..Default::default()
        });

        worker_states.lock().unwrap().insert(
            entry.module_name.clone(),
            WorkerStates {
                alive: is_alive,
                on_crash: false,
            },
        );

        std::thread::sleep(std::time::Duration::from_secs(
            entry.stats.delay.unwrap_or(60),
        ));
    }
}
