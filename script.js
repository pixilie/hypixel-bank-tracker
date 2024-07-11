const WS_READY_STATE_CONNECTING = 0;
const WS_READY_STATE_OPEN = 1;
const WS_READY_STATE_CLOSING = 2;
const WS_READY_STATE_CLOSED = 3;

// ———

const reloadButton = document.querySelector("button#reload");
if (!reloadButton) throw new Error("could not select reload button");
const websocketInfoStatus = document.querySelector("#technical-infos #websocket-info span");
if (!websocketInfoStatus) throw new Error("could not select websocket info status");

// ———

let ws = new WebSocket(`ws://${window.location.host}/ws`);

ws.addEventListener("open", _ => {
  console.info("[RELOAD] Connected");

  websocketInfoStatus.classList.remove("disconnected")
  websocketInfoStatus.innerText = "connected";
  websocketInfoStatus.classList.add("connected")
});
ws.addEventListener("close", _ => {
  console.info("[RELOAD] Disconnected")

  websocketInfoStatus.classList.remove("connected")
  websocketInfoStatus.innerText = "disconnected";
  websocketInfoStatus.classList.add("disconnected")
});
ws.addEventListener("error", console.error);

ws.addEventListener("message", event => {
  console.info("[RELOAD] Connected")

  if (event.data === "reload") {
    window.location.reload();
  } else {
    console.error("Unknown message recieved")
  };
});

// ———

reloadButton.addEventListener("click", (ev) => {
  ev.preventDefault();

  switch (ws.readyState) {
    case WS_READY_STATE_CONNECTING:
    case WS_READY_STATE_CLOSING:
      break;
    case WS_READY_STATE_OPEN:
      ws.send("reload");
      break;
    case WS_READY_STATE_CLOSED:
      window.location.reload();
      break;
  }
});

