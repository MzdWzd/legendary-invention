use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel::<String>(100);
    let history = Arc::new(Mutex::new(Vec::<String>::new()));

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

    let port = std::env::var("PORT").unwrap_or("10000".into());
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse().unwrap()));

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
<meta charset="utf-8">
<title>Rust GC</title>
<style>
body {
  margin: 0;
  font-family: Arial, sans-serif;
  background: #2b2d31;
  color: #dcddde;
  display: flex;
  flex-direction: column;
  height: 100vh;
}

#chat {
  flex: 1;
  overflow-y: auto;
  padding: 15px;
}

.message {
  margin-bottom: 8px;
}

.username {
  font-weight: bold;
  color: #7289da;
  margin-right: 5px;
}

#input-bar {
  display: flex;
  padding: 10px;
  background: #1e1f22;
}

#name {
  width: 120px;
  margin-right: 8px;
}

#msg {
  flex: 1;
}

input {
  background: #383a40;
  border: none;
  color: #dcddde;
  padding: 8px;
  border-radius: 4px;
}
</style>
</head>
<body>

<div id="chat"></div>

<div id="input-bar">
  <input id="name" placeholder="Username">
  <input id="msg" placeholder="Message">
</div>

<script>
let ws = new WebSocket(`wss://${location.host}/ws`);

const chat = document.getElementById("chat");
const nameInput = document.getElementById("name");
const msgInput = document.getElementById("msg");

ws.onmessage = e => {
  const div = document.createElement("div");
  div.className = "message";

  const [name, ...rest] = e.data.split(": ");
  const msg = rest.join(": ");

  div.innerHTML =
    '<span class="username">' + name + '</span>' +
    '<span>' + msg + '</span>';

  chat.appendChild(div);
  chat.scrollTop = chat.scrollHeight;
};

function send() {
  const name = nameInput.value.trim();
  const msg = msgInput.value.trim();
  if (!name || !msg) return;

  ws.send(name + ": " + msg);
  msgInput.value = "";
}

msgInput.addEventListener("keydown", e => {
  if (e.key === "Enter") {
    e.preventDefault();
    send();
  }
});
</script>

</body>
</html>
"#)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    tx: broadcast::Sender<String>,
    history: Arc<Mutex<Vec<String>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx, history))
}

async fn handle_socket(
    socket: WebSocket,
    tx: broadcast::Sender<String>,
    history: Arc<Mutex<Vec<String>>>,
) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // send history
    {
        let hist = history.lock().await;
        for msg in hist.iter() {
            let _ = sender.send(Message::Text(msg.clone())).await;
        }
    }

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = sender.send(Message::Text(msg)).await;
        }
    });

    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        history.lock().await.push(text.clone());
        let _ = tx.send(text);
    }

    send_task.abort();
}
