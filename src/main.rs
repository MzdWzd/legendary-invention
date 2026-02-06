use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
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
        .route("/ws", get({
            let tx = tx.clone();
            move |ws| ws_handler(ws, tx.clone())
        }));

    let port = std::env::var("PORT")
        .unwrap_or("3000".into())
        .parse::<u16>()
        .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Listening on {}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
    .await
    .unwrap();
}

async fn index() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>Rust GC</title>
<style>
body {
  margin: 0;
  background: #1e1f22;
  color: #dcddde;
  font-family: sans-serif;
}
#chat {
  height: 90vh;
  overflow-y: auto;
  padding: 10px;
}
.msg {
  margin-bottom: 6px;
}
#input {
  display: flex;
  padding: 10px;
  background: #2b2d31;
}
input {
  flex: 1;
  background: #383a40;
  border: none;
  color: white;
  padding: 10px;
}
input:focus { outline: none; }
</style>
</head>
<body>

<div id="chat"></div>

<div id="input">
  <input id="box" placeholder="Type message and press Enter…" />
</div>

<script>
let ws;
let username = prompt("Username:");

function connect() {
  ws = new WebSocket(
    (location.protocol === "https:" ? "wss://" : "ws://") +
    location.host + "/ws"
  );

  ws.onopen = () => {
    ws.send(username);
  };

  ws.onmessage = e => {
    const div = document.createElement("div");
    div.className = "msg";
    div.textContent = e.data;
    document.getElementById("chat").appendChild(div);
    document.getElementById("chat").scrollTop =
      document.getElementById("chat").scrollHeight;
  };
}

document.getElementById("box").addEventListener("keydown", e => {
  if (e.key === "Enter" && ws.readyState === 1) {
    ws.send(e.target.value);
    e.target.value = "";
  }
});

connect();
</script>

</body>
</html>
"#)
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
