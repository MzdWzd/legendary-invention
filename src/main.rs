use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
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
        .route("/ws", get(move |ws| ws_handler(ws, tx.clone())));

    let port = std::env::var("PORT").unwrap_or("3000".into());
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse().unwrap()));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn index() -> impl IntoResponse {
    r#"
<!DOCTYPE html>
<html>
<body>
<h2>Rust Chat</h2>
<input id="name" placeholder="Username"><br>
<input id="msg" placeholder="Message">
<button onclick="send()">Send</button>
<ul id="chat"></ul>

<script>
let ws;
function send() {
  if (!ws) {
    ws = new WebSocket(`wss://${location.host}/ws`);
    ws.onmessage = e => {
      const li = document.createElement("li");
      li.textContent = e.data;
      document.getElementById("chat").appendChild(li);
    };
  }
  ws.onopen = () => {
    const name = document.getElementById("name").value;
    const msg = document.getElementById("msg").value;
    ws.send(name + ": " + msg);
  };
  if (ws.readyState === 1) {
    ws.send(
      document.getElementById("name").value +
      ": " +
      document.getElementById("msg").value
    );
  }
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
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

async fn handle_socket(mut socket: WebSocket, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();

    let (mut sender, mut receiver) = socket.split();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = sender.send(Message::Text(msg)).await;
        }
    });

    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        let _ = tx.send(text);
    }
}
