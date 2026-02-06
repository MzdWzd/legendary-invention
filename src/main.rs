use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel::<String>(100);

    let app = Router::new()
        .route("/", get(index))
        .route(
            "/ws",
            get(move |ws: WebSocketUpgrade| {
                let tx = tx.clone();
                async move {
                    ws_handler(ws, tx).await
                }
            }),
        );

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Listening on {}", addr);

    // Axum 0.7 server startup (REQUIRED)
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> impl IntoResponse {
    r#"
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Rust Chat</title>
</head>
<body>
<h2>Rust Chat</h2>

<input id="name" placeholder="Username"><br>
<input id="msg" placeholder="Message">
<button onclick="send()">Send</button>

<ul id="chat"></ul>

<script>
const ws = new WebSocket(
  (location.protocol === "https:" ? "wss://" : "ws://") +
  location.host +
  "/ws"
);

ws.onmessage = e => {
  const li = document.createElement("li");
  li.textContent = e.data;
  document.getElementById("chat").appendChild(li);
};

function send() {
  const name = document.getElementById("name").value || "anon";
  const msg = document.getElementById("msg").value;
  ws.send(name + ": " + msg);
}
</script>


</body>
</html>
"#
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    tx: broadcast::Sender<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_socket(socket, tx).await
    })
}

async fn handle_socket(socket: WebSocket, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Broadcast → client
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = sender.send(Message::Text(msg)).await;
        }
    });

    // Client → broadcast
    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        let _ = tx.send(text);
    }
}
