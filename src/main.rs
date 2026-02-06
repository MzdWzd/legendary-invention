use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;
use tokio::{fs, net::TcpListener};

type History = Arc<Mutex<Vec<String>>>;
const HISTORY_FILE: &str = "chat.json";

#[tokio::main]
async fn main() {
    let history: Vec<String> = fs::read_to_string(HISTORY_FILE)
        .await
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();

    let state = Arc::new(Mutex::new(history));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
        .nest_service("/", ServeDir::new("static"));

    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    let listener = TcpListener::bind(addr).await.unwrap();

    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(history): State<History>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| socket_handler(socket, history))
}

async fn socket_handler(mut socket: WebSocket, history: History) {
    // clone history BEFORE await (Send-safe)
    let past = {
        history.lock().unwrap().clone()
    };

    for msg in past {
        let _ = socket.send(Message::Text(msg)).await;
    }

    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        let snapshot = {
            let mut h = history.lock().unwrap();
            h.push(text.clone());
            h.clone()
        };

        let _ = fs::write(
            HISTORY_FILE,
            serde_json::to_string(&snapshot).unwrap(),
        ).await;

        let _ = socket.send(Message::Text(text)).await;
    }
}
