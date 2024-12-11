use std::io::Read;

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
        Ok(_) => {}
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

    let mut wasm_bytes = vec![];
    bar.set_message("Reading files in MODULES_PATH folder");
    for entry in modules_path_iterator {
        let entry = entry.expect("Error: Could not read entry in MODULES_PATH folder");

        let entry_path = entry.path();

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
        wasm_bytes.push(
            std::fs::read(entry_path).expect("Error: Could not read file in MODULES_PATH folder"),
        );
    }

    if wasm_bytes.is_empty() {
        bar.finish_with_message("No modules found, exiting...");
        return;
    }

    bar.finish_with_message("Finished reading modules");

    // let wasm_tasks = vec![];
    for entry in wasm_bytes {
        std::thread::spawn(move || {
            // Create a Store.
            let mut store = Store::default();
            // Let's compile the Wasm module.
            let module = match Module::new(&store, &entry) {
                Ok(val) => val,
                Err(_) => {
                    panic!("Error: Could not compile Wasm module");
                }
            };

            let (_, stdout_rx) = Pipe::channel();

            let stdout_rx = Box::new(stdout_rx);
            let module = Box::new(module);
            loop {
                let mut stdout_rx = stdout_rx.clone();
                let module = module.clone();
                // Run the module.
                let _ = WasiEnv::builder("hello")
                    // .args(&["world"])
                    // .env("KEY", "Value")
                    .stdout(stdout_rx.clone())
                    .run_with_store(*module, &mut store);

                let mut buf = String::new();
                stdout_rx.read_to_string(&mut buf).unwrap();

                println!("{}", buf);
                drop(stdout_rx);
            }
        });
    }

    std::thread::park();
}
