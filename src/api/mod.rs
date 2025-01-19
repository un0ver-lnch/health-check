use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};

use crate::types::{NativeStates, NativeWorkerStates, RunnerState, WorkerStates};

struct AppState {
    worker_states: Arc<Mutex<HashMap<String, WorkerStates>>>,
    native_worker_states: Arc<Mutex<HashMap<String, NativeWorkerStates>>>,
    runner_states: Arc<Mutex<HashMap<String, RunnerState>>>,
    native_states: Arc<Mutex<HashMap<String, NativeStates>>>,
}

#[tokio::main]
pub async fn create_server(
    worker_states: Arc<Mutex<HashMap<String, WorkerStates>>>,
    native_worker_states: Arc<Mutex<HashMap<String, NativeWorkerStates>>>,
    runner_states: Arc<Mutex<HashMap<String, RunnerState>>>,
    native_states: Arc<Mutex<HashMap<String, NativeStates>>>,
) {
    let app_state = AppState {
        worker_states,
        native_worker_states,
        runner_states,
        native_states,
    };
    // build our application with a single route
    let app = Router::new()
        .route("/health/:service_name", get(get_health))
        .route("/health/lib/:service_name", get(get_lib_health))
        .route("/thunder/lib/:service_name", post(run_lib_service_thunder))
        .route("/thunder/:service_name", post(run_service_thunder))
        .route("/thunder/stats/:service_name", get(get_service_stats))
        .with_state(Arc::new(Mutex::new(app_state)));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_health(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> (StatusCode, String) {
    let state = match state.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    let worker_states = match state.worker_states.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    // println!("Service name: {}", service_name);
    // println!("State: {:?}", state);

    if let Some(worker_state) = worker_states.get(&service_name) {
        if worker_state.on_crash {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Health service is not available".to_string(),
            );
        } else if worker_state.alive {
            return (StatusCode::OK, "OK".to_string());
        } else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service is not available".to_string(),
            );
        }
    } else {
        return (StatusCode::NOT_FOUND, "Service not found".to_string());
    };
}

async fn get_lib_health(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> (StatusCode, String) {
    let state = match state.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    let native_worker_states = match state.native_worker_states.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    if let Some(native_worker_state) = native_worker_states.get(&service_name) {
        if native_worker_state.on_crash {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Health service is not available".to_string(),
            );
        } else if native_worker_state.alive {
            return (StatusCode::OK, "OK".to_string());
        } else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service is not available".to_string(),
            );
        }
    } else {
        return (StatusCode::NOT_FOUND, "Service not found".to_string());
    };
}

async fn run_service_thunder(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> (StatusCode, String) {
    let state = match state.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    let runner_state = match state.runner_states.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    // Schedule a run for the service.

    if let Some(runner_state) = runner_state.get(&service_name) {
        // Check if the service is already running.
        match runner_state.channel_trigger.send(()) {
            Ok(_) => {
                return (StatusCode::OK, "Service is running".to_string());
            }
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error sending trigger to service".to_string(),
                );
            }
        }
    } else {
        return (StatusCode::NOT_FOUND, "Service not found".to_string());
    }
}

async fn run_lib_service_thunder(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> (StatusCode, String) {
    let state = match state.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    let native_state = match state.native_states.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    // Schedule a run for the service.

    if let Some(native_state) = native_state.get(&service_name) {
        // Check if the service is already running.
        match native_state.channel_trigger.send(()) {
            Ok(_) => {
                return (StatusCode::OK, "Service is running".to_string());
            }
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error sending trigger to service".to_string(),
                );
            }
        }
    } else {
        return (StatusCode::NOT_FOUND, "Service not found".to_string());
    }
}

async fn get_service_stats(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> (StatusCode, String) {
    let state = match state.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    let runner_state = match state.runner_states.lock() {
        Ok(val) => val,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error getting lock".to_string(),
            );
        }
    };

    if let Some(runner_state) = runner_state.get(&service_name) {
        return (
            StatusCode::OK,
            format!(
                "Service: {}\nLast run: {:?}\nLast run success: {}\n",
                runner_state.module_name, runner_state.last_run, runner_state.last_run_success
            ),
        );
    } else {
        return (StatusCode::NOT_FOUND, "Service not found".to_string());
    }
}
