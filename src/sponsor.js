// sponsor popup — click-anywhere / Esc to dismiss, i18n, broken-image fallback.
// Exit animation runs here (100ms), then Rust closes the window.
const { invoke } = window.__TAURI__.core;

// i18n (page-level; system locale via navigator, no IPC needed)
const zh = navigator.language.toLowerCase().startsWith("zh");
const T = zh ? {} : {
  title: "Buy Me a Coffee",
  sub: "Scan with WeChat · tips welcome",
  hint: "Click anywhere to dismiss · Esc",
  fallback: "QR failed to load —<br/>see GitHub README",
  aria: "WeChat tip QR code",
};
if (!zh) {
  document.getElementById("t-title").textContent = T.title;
  document.getElementById("t-sub").textContent = T.sub;
  document.getElementById("t-hint").textContent = T.hint;
  document.getElementById("qrfallback").innerHTML = T.fallback;
  document.getElementById("card").setAttribute("aria-label", T.aria);
}

// broken image: swap to text fallback, card keeps its size
document.getElementById("qr").addEventListener("error", () => {
  document.getElementById("qrwrap").classList.add("broken");
});

// single-shot dismiss: play the fade, then ask Rust to close
let closing = false;
function dismiss() {
  if (closing) return;
  closing = true;
  document.getElementById("card").classList.add("closing");
  setTimeout(() => invoke("close_sponsor").catch(() => {}), 100);
}
window.addEventListener("click", dismiss);
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") dismiss();
});

// Esc needs DOM focus on the card
window.addEventListener("load", () => document.getElementById("card").focus());
