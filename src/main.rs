use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    Router,
};
use chrono::Local;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{fs, net::TcpListener};
use tower_http::services::ServeDir;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};

type Clients = Arc<Mutex<Vec<axum::extract::ws::WebSocketSender>>>;
type Users = Arc<Mutex<HashMap<String, String>>>;
type History = Arc<Mutex<Vec<String>>>;

const USERS_FILE: &str = "users.json";
const HISTORY_FILE: &str = "chat.json";

#[derive(Deserialize)]
struct Auth {
    username: String,
    password: String,
}

#[tokio::main]
async fn main() {
    let users: Users = Arc::new(Mutex::new(
        fs::read_to_string(USERS_FILE)
            .await
            .ok()
            .and_then(|d| serde_json::from_str(&d).ok())
            .unwrap_or_default(),
    ));

    let history: History = Arc::new(Mutex::new(
        fs::read_to_string(HISTORY_FILE)
            .await
            .ok()
            .and_then(|d| serde_json::from_str(&d).ok())
            .unwrap_or_default(),
    ));

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state((clients, users, history))
        .nest_service("/", ServeDir::new("static"));

    let listener = TcpListener::bind("0.0.0.0:10000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State((clients, users, history)): State<(Clients, Users, History)>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| socket_handler(socket, clients, users, history))
}

async fn socket_handler(
    mut socket: WebSocket,
    clients: Clients,
    users: Users,
    history: History,
) {
    // AUTH FIRST
    let auth = match socket.recv().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<Auth>(&t).ok(),
        _ => None,
    };
    let auth = match auth {
        Some(a) => a,
        None => return,
    };

    let mut users_lock = users.lock().unwrap();
    if let Some(hash) = users_lock.get(&auth.username) {
        let parsed = PasswordHash::new(hash).unwrap();
        if Argon2::default()
            .verify_password(auth.password.as_bytes(), &parsed)
            .is_err()
        {
            let _ = socket.send(Message::Text("AUTH_FAIL".into())).await;
            return;
        }
    } else {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(auth.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        users_lock.insert(auth.username.clone(), hash);
        let _ = fs::write(USERS_FILE, serde_json::to_string(&*users_lock).unwrap()).await;
    }
    drop(users_lock);

    let _ = socket.send(Message::Text("AUTH_OK".into())).await;

    // send history
    for msg in history.lock().unwrap().iter() {
        let _ = socket.send(Message::Text(msg.clone())).await;
    }

    let sender = socket.sender();
    clients.lock().unwrap().push(sender);

    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        let time = Local::now().format("%H:%M").to_string();
        let line = format!("[{}] {}: {}", time, auth.username, text);

        {
            let mut h = history.lock().unwrap();
            h.push(line.clone());
            let _ = fs::write(HISTORY_FILE, serde_json::to_string(&*h).unwrap()).await;
        }

        let mut dead = Vec::new();
        let mut cls = clients.lock().unwrap();
        for (i, c) in cls.iter_mut().enumerate() {
            if c.send(Message::Text(line.clone())).await.is_err() {
                dead.push(i);
            }
        }
        for i in dead.into_iter().rev() {
            cls.remove(i);
        }
    }
}
