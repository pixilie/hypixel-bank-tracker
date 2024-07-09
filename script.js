let ws = new WebSocket(`ws://${window.location.host}/ws`);

ws.addEventListener("open", _ => console.info("[RELOAD] Connected"));
ws.addEventListener("close", _ => console.info("[RELOAD] Disconnected"));
ws.addEventListener("error", console.error);

ws.addEventListener("message", event => {
  console.info("[RELOAD] Connected")

  if (event.data === "reload") {
    window.location.reload();
  } else {
    console.error("Unknown message recieved")
  };
});

// ---

const reloadButton = document.querySelector("button#reload");

reloadButton.addEventListener("click", (ev) => {
  ev.preventDefault();
  ws.send("reload");
});

