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
    let mut store = Store::default();
    let module = match Module::new(&store, &entry.bytes) {
        Ok(val) => Box::new(val),
        Err(err) => {
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

    loop {
        let (stdout_tx, mut stdout_rx) = Pipe::channel();
        let builder = WasiEnv::builder(&entry.module_name)
            .stdout(Box::new(stdout_tx))
            .run_with_store(*module.clone(), &mut store);

        if builder.is_err() {
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
        stdout_rx.read_to_string(&mut buf).unwrap();

        worker_states.lock().unwrap().insert(
            entry.module_name.clone(),
            WorkerStates {
                alive: buf.eq("true"),
                on_crash: false,
            },
        );

        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
