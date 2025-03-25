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

let ws = new WebSocket(`wss://${window.location.host}/ws`);

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

// ———
const transferForm = document.querySelector("form#transaction-form");
if (!transferForm) throw new Error("Could not select transfer form");
const logsSpan = document.querySelector("span#transfer-logs")
if (!logsSpan) throw new Error("Could not select logs span")
const dataList = document.querySelector("datalist#users-list")
if (!dataList) throw new Error("Could not select datalist")
// ———

transferForm.addEventListener("submit", async (ev) => {
  ev.preventDefault();

  let form = new FormData(transferForm);
  let amount = parseInt(form.get("amount"));
  let fromUser = form.get("from-user");
  let toUser = form.get("to-user");

  let userList = Array.from(dataList.options).map((elt) => elt.label)

  if (amount <= 0) {
    logsSpan.innerText = "Invalid amount: Must be greater than 0."
  } else if (!userList.includes(fromUser) || !userList.includes(toUser)) {
    logsSpan.innerText = "Invalid user: Unknown user"
  } else {
    ws.send(`transfer;${amount};${fromUser};${toUser}`);
  }
});
