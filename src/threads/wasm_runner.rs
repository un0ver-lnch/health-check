use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
};

use sqlite::Connection;
use wasmer::{Module, Store};
use wasmer_wasix::{Pipe, WasiEnv};

use crate::{
    persistency::{self, Save},
    types::{RunnerState, WasmRunner},
};

pub fn spawn_wasm_runner_threads(
    wasm_run_containers: Vec<WasmRunner>,
    runner_states: Arc<Mutex<HashMap<String, RunnerState>>>,
    runner_connection: Arc<Mutex<Connection>>,
) {
    for runner in wasm_run_containers {
        let runner_states = runner_states.clone();
        let runner_connection = runner_connection.clone();
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
            run_wasm_module(runner, runner_states, runner_connection, channel_reciver)
        });
    }
}

fn run_wasm_module(
    runner: WasmRunner,
    runner_states: Arc<Mutex<HashMap<String, RunnerState>>>,
    runner_connection: Arc<Mutex<Connection>>,
    channel_reciver: std::sync::mpsc::Receiver<()>,
) {
    let mut store = Store::default();
    let module = match Module::new(&store, &runner.bytes) {
        Ok(val) => Box::new(val),
        Err(_) => {
            if let Ok(mut states) = runner_states.lock() {
                if let Some(state) = states.get_mut(&runner.module_name) {
                    state.last_run_success = false;
                }
            }
            return;
        }
    };

    while let Ok(_) = channel_reciver.recv() {
        process_wasm_execution(
            &runner,
            &runner_states,
            &runner_connection,
            &mut store,
            &module,
        );
    }
}

fn process_wasm_execution(
    runner: &WasmRunner,
    runner_states: &Arc<Mutex<HashMap<String, RunnerState>>>,
    runner_connection: &Arc<Mutex<Connection>>,
    store: &mut Store,
    module: &Box<Module>,
) {
    let (stdout_tx, mut stdout_rx) = Pipe::channel();
    let (stderr_tx, mut stderr_rx) = Pipe::channel();

    let builder = WasiEnv::builder(&runner.module_name)
        .stdout(Box::new(stdout_tx))
        .stderr(Box::new(stderr_tx))
        .run_with_store(*module.clone(), store);

    if builder.is_err() {
        if let Ok(mut states) = runner_states.lock() {
            if let Some(state) = states.get_mut(&runner.module_name) {
                state.last_run_success = false;
            }
        }
        return;
    }

    let mut stdout = String::new();
    stdout_rx.read_to_string(&mut stdout).unwrap();

    process_output(&stdout, runner_connection);

    let mut stderr = String::new();
    stderr_rx.read_to_string(&mut stderr).unwrap();

    for line in stderr.lines() {
        println!("RUNNER {}: {}", &runner.module_name, line);
    }
}

fn process_output(output: &str, connection: &Arc<Mutex<Connection>>) {
    for line in output.lines() {
        if !line.starts_with("KV:") {
            continue;
        }

        let filtered_data = line.replace("KV:", "");
        let parts: Vec<&str> = filtered_data.split("###").collect();

        if parts.len() != 2 {
            continue;
        }

        let key_value_pair = persistency::KeyValuePair {
            key: parts[0].to_string(),
            value: parts[1].to_string(),
        };

        if let Ok(_) = key_value_pair.persist(&connection.lock().unwrap()) {
            println!(
                "Persisted key: {} with value: {}",
                &key_value_pair.key, &key_value_pair.value
            );
        }
    }
}
