#[derive(Debug)]
pub struct WasmWorker {
    pub module_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct WorkerStates {
    pub on_crash: bool,
    pub alive: bool,
}
