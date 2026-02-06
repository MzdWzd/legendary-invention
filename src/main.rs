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
            move |ws| ws_handler(ws, tx)
        }));

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
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
  <title>Rust Group Chat</title>
  <style>
    body {
      font-family: sans-serif;
      max-width: 600px;
      margin: auto;
      background: #111;
      color: #eee;
    }
    #chat {
      border: 1px solid #444;
      height: 300px;
      overflow-y: auto;
      padding: 8px;
      margin-bottom: 8px;
      background: #1e1e1e;
    }
    input, button {
      width: 100%;
      margin: 4px 0;
      padding: 6px;
      background: #222;
      color: #eee;
      border: 1px solid #444;
    }
  </style>
</head>
<body>
  <h2>Group Chat</h2>

  <div id="chat"></div>

  <input id="name" placeholder="Username">
  <input id="msg" placeholder="Message">
  <button onclick="send()">Send</button>

  <script>
    const ws = new WebSocket(
      (location.protocol === "https:" ? "wss://" : "ws://") +
      location.host + "/ws"
    );

    ws.onmessage = e => {
      const div = document.createElement("div");
      div.textContent = e.data;
      document.getElementById("chat").appendChild(div);
    };

    function send() {
      const name = document.getElementById("name").value || "anon";
      const msg = document.getElementById("msg").value;
      ws.send(name + ": " + msg);
      document.getElementById("msg").value = "";
    }
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

async fn handle_socket(socket: WebSocket, tx: broadcast::Sender<String>) {
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
