use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

type History = Arc<Mutex<Vec<String>>>;

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel::<String>(100);
    let history: History = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/", get(index))
        .route(
            "/ws",
            get({
                let tx = tx.clone();
                let history = history.clone();
                move |ws| ws_handler(ws, tx.clone(), history.clone())
            }),
        );

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> impl IntoResponse {
    r#"
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Rust GC</title>
<style>
body { font-family: sans-serif; background:#111; color:#eee; }
#chat { list-style:none; padding:0; }
li { margin:4px 0; }
input { margin:4px 0; }
</style>
</head>
<body>
<h2>Rust Group Chat</h2>
<input id="name" placeholder="Username"><br>
<input id="msg" placeholder="Message">
<button onclick="send()">Send</button>
<ul id="chat"></ul>

<script>
let ws = new WebSocket(`wss://${location.host}/ws`);

ws.onmessage = e => {
  const li = document.createElement("li");
  li.textContent = e.data;
  document.getElementById("chat").appendChild(li);
};

function send() {
  const name = document.getElementById("name").value;
  const msg = document.getElementById("msg").value;
  ws.send(JSON.stringify({ name, msg }));
}
</script>
</body>
</html>
"#
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    tx: broadcast::Sender<String>,
    history: History,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx, history))
}

async fn handle_socket(
    socket: WebSocket,
    tx: broadcast::Sender<String>,
    history: History,
) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Send chat history to new client
    {
        let hist = history.lock().unwrap();
        for msg in hist.iter() {
            let _ = sender.send(Message::Text(msg.clone())).await;
        }
    }

    let mut username: Option<String> = None;

    // Task: broadcast messages to this socket
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = sender.send(Message::Text(msg)).await;
        }
    });

    // Receive messages from this socket
    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let (Some(name), Some(msg)) =
                (value.get("name"), value.get("msg"))
            {
                let name = name.as_str().unwrap_or("anon");
                let msg = msg.as_str().unwrap_or("");

                username = Some(name.to_string());
                let full = format!("{}: {}", name, msg);

                history.lock().unwrap().push(full.clone());
                let _ = tx.send(full);
            }
        }
    }

    send_task.abort();
}
