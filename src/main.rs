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

type History = Arc<Mutex<Vec<String>>>;

#[tokio::main]
async fn main() {
    let history: History = Arc::new(Mutex::new(Vec::new()));

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
    // send history
    let past = history.lock().unwrap().clone();
    for msg in past {
        let _ = socket.send(Message::Text(msg)).await;
    }

    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        history.lock().unwrap().push(text.clone());
        let _ = socket.send(Message::Text(text)).await;
    }
}
