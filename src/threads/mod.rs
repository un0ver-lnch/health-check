mod dll_runner;
mod dll_worker;
mod wasm_runner;
mod wasm_worker;

pub use dll_runner::spawn_dll_runner_threads;
pub use dll_worker::spawn_dll_worker_threads;
pub use wasm_runner::spawn_wasm_runner_threads;
pub use wasm_worker::spawn_wasm_worker_threads;
