use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Router,
};

use crate::types::WorkerStates;

#[tokio::main]
pub async fn create_server(worker_states: Arc<Mutex<HashMap<String, WorkerStates>>>) {
    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/health/:service_name", get(get_health))
        .with_state(worker_states);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_health(
    Path(service_name): Path<String>,
    State(state): State<Arc<Mutex<HashMap<String, WorkerStates>>>>,
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

    // println!("Service name: {}", service_name);
    // println!("State: {:?}", state);

    if let Some(worker_state) = state.get(&service_name) {
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
