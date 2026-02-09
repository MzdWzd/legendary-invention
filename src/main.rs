<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<title>GC</title>
<style>
body { margin:0; font-family:sans-serif; background:#2b2d31; color:white }
#chat { height:90vh; overflow-y:auto; padding:10px }
#input { display:flex }
input { flex:1; padding:10px; background:#1e1f22; color:white; border:none }
</style>
</head>
<body>

<div id="chat"></div>
<div id="input">
<input id="msg" placeholder="message..." />
</div>

<script>
const user = prompt("username");
const pass = prompt("password");
const ws = new WebSocket(`ws://${location.host}/ws`);
const chat = document.getElementById("chat");
const input = document.getElementById("msg");

ws.onopen = () => {
  ws.send(JSON.stringify({ username:user, password:pass }));
};

ws.onmessage = e => {
  if (e.data === "AUTH_FAIL") {
    alert("bad password"); return;
  }
  if (e.data === "AUTH_OK") return;
  const div = document.createElement("div");
  div.textContent = e.data;
  chat.appendChild(div);
  chat.scrollTop = chat.scrollHeight;
};

input.onkeydown = e => {
  if (e.key === "Enter" && input.value.trim()) {
    ws.send(input.value);
    input.value = "";
  }
};
</script>
</body>
</html>
