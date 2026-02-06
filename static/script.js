const ws = new WebSocket(
  (location.protocol === "https:" ? "wss://" : "ws://") +
  location.host + "/ws"
);

const messages = document.getElementById("messages");
const input = document.getElementById("input");
const usernameInput = document.getElementById("username");

ws.onmessage = (e) => {
  const div = document.createElement("div");
  div.textContent = e.data;
  messages.appendChild(div);
  messages.scrollTop = messages.scrollHeight;
};

input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && input.value) {
    const name = usernameInput.value || "anon";
    ws.send(`${name}: ${input.value}`);
    input.value = "";
  }
});
