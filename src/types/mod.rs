#[derive(Debug)]
pub struct WasmWorker {
    pub module_name: String,
    pub bytes: Vec<u8>,
}

pub struct WasmRunner {
    pub module_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct WorkerStates {
    pub on_crash: bool,
    pub alive: bool,
}

pub struct RunnerState {
    pub module_name: String,
    pub last_run: std::time::Instant,
    pub last_run_success: bool,
    pub channel_trigger: std::sync::mpsc::Sender<()>,
}
