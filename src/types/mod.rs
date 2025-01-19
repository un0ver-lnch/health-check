#[derive(Debug)]
pub struct WasmWorker {
    pub module_name: String,
    pub bytes: Vec<u8>,
}

pub struct WasmRunner {
    pub module_name: String,
    pub bytes: Vec<u8>,
}

pub struct DLLRunner {
    pub module_name: String,
    pub path: String,
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

pub struct NativeWorkerStates {
    pub on_crash: bool,
    pub alive: bool,
}

pub struct NativeStates {
    pub on_crash: bool,
    pub last_run: std::time::Instant,
    pub last_run_success: bool,
    pub channel_trigger: std::sync::mpsc::Sender<()>,
}
