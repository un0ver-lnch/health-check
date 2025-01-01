mod api;

mod types;

use types::{RunnerState, WasmRunner, WasmWorker, WorkerStates};

use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
};

use indicatif::ProgressBar;
use wasmer::{Module, Store};

use wasmer_wasix::{Pipe, WasiEnv};

fn main() {
    let bar = ProgressBar::new_spinner();
    let modules_folder_path = match std::env::var("MODULES_PATH") {
        Ok(val) => val,
        Err(_) => {
            panic!("Error: MODULES_PATH env variable not set");
        }
    };
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

        if entry.file_name().to_str().unwrap().ends_with(".wasm") == false {
            continue;
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
        } else {
            wasm_containers.push(WasmWorker {
                module_name: entry.file_name().to_str().unwrap().to_string(),
                bytes: std::fs::read(entry_path)
                    .expect("Error: Could not read file in MODULES_PATH folder"),
            });
        }
    }

    if wasm_containers.is_empty() {
        bar.finish_with_message("No modules found, exiting...");
        return;
    }

    bar.finish_with_message("Finished reading modules");

    let worker_states = Arc::new(Mutex::new(HashMap::new()));

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

    let runner_states = Arc::new(Mutex::new(HashMap::new()));

    for runner in wasm_run_containers {
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

                let mut bur_err = String::new();
                stderr_rx.read_to_string(&mut bur_err).unwrap();

                let stderr_split = bur_err.split("\n").collect::<Vec<&str>>();

                for err_line in stderr_split {
                    if err_line.starts_with("KV:") {}
                }

                // println!("Read \"{}\" from the WASI stdout!", buf.trim());
                // println!("{} == {} = {}", buf, "true", buf.trim().eq("true"));
            }
        });
    }

    std::thread::spawn(move || {
        //let worker_states = worker_states.clone();
        api::create_server(worker_states, runner_states);
    });

    std::thread::park();
}
