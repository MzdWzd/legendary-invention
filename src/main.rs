use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

static HISTORY: Lazy<Arc<Mutex<Vec<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel::<String>(100);

    let app = Router::new()
        .route("/", get(index))
        .route(
            "/ws",
            get(move |ws: WebSocketUpgrade| {
                let tx = tx.clone();
                let history = HISTORY.clone();
                async move { ws.on_upgrade(move |socket| handle_socket(socket, tx, history)) }
            }),
        );

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app).await.unwrap();
}

async fn handle_socket(
    socket: WebSocket,
    tx: broadcast::Sender<String>,
    history: Arc<Mutex<Vec<String>>>,
) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Send history first
    {
        let hist = history.lock().unwrap().clone();
        for msg in hist {
            let _ = sender.send(Message::Text(msg)).await;
        }
    }

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let _ = sender.send(Message::Text(msg)).await;
        }
    });

    let recv_task = tokio::spawn(async move {
        let mut username: Option<String> = None;

        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            if username.is_none() {
                username = Some(text.clone());
                continue;
            }

            let msg = format!("{}: {}", username.clone().unwrap(), text);
            let _ = tx.send(msg.clone());

            let mut hist = HISTORY.lock().unwrap();
            hist.push(msg);
            if hist.len() > 200 {
                hist.remove(0);
            }
        }
    });

    let _ = tokio::join!(send_task, recv_task);
}

async fn index() -> impl IntoResponse {
    r#"
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
"#
}
