use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;
use tokio::fs;

type History = Arc<Mutex<Vec<String>>>;

const HISTORY_FILE: &str = "chat.json";

#[tokio::main]
async fn main() {
    // load history from disk
    let history: Vec<String> = match fs::read_to_string(HISTORY_FILE).await {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    let history: History = Arc::new(Mutex::new(history));

    let app = Router::new()
        .route("/ws", get({
            let history = history.clone();
            move |ws: WebSocketUpgrade| handle_ws(ws, history.clone())
        }))
        .nest_service("/", ServeDir::new("static"));

    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    println!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn handle_ws(ws: WebSocketUpgrade, history: History) -> impl IntoResponse {
    ws.on_upgrade(move |socket| socket_handler(socket, history))
}

async fn socket_handler(mut socket: WebSocket, history: History) {
    // send saved history
    let past = history.lock().unwrap().clone();
    for msg in past {
        let _ = socket.send(Message::Text(msg)).await;
    }

    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        {
            let mut hist = history.lock().unwrap();
            hist.push(text.clone());

            // save to disk
            let _ = fs::write(
                HISTORY_FILE,
                serde_json::to_string(&*hist).unwrap(),
            )
            .await;
        }

        let _ = socket.send(Message::Text(text)).await;
    }
}
