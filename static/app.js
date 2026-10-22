// Renders the dashboard in the browser, for the static site (src/site.rs). The page arrives
// showing the default view. This loads the dataset and the WebAssembly build of the same Rust
// renderer the server uses (src/wasm.rs), then draws every other view here, so links, forms and
// the CSV download work without a server. URLs are the same as the server's.

const tag = document.querySelector("script[data-wasm]");
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let wasm;
let failed = false;

const ready = (async () => {
  const [module, data] = await Promise.all([
    WebAssembly.compileStreaming(fetch(tag.dataset.wasm)),
    fetch(tag.dataset.data).then((r) => {
      if (!r.ok) throw new Error(`${r.status} loading ${r.url}`);
      return r.arrayBuffer();
    }),
  ]);
  wasm = (await WebAssembly.instantiate(module)).exports;
  if (!call(wasm.load, new Uint8Array(data))) throw new Error(output());
})();
ready.catch((e) => {
  // Without the renderer, links fall back to plain navigation, which shows the default view.
  failed = true;
  document.documentElement.classList.remove("loading");
  console.error("Couldn't load the dashboard:", e);
});

// Copies `bytes` into the module's memory and calls `fn` with them; see src/wasm.rs.
function call(fn, bytes) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  return fn(ptr, bytes.length);
}

function outputBytes() {
  return new Uint8Array(wasm.memory.buffer, wasm.out_ptr(), wasm.out_len()).slice();
}

function output() {
  return decoder.decode(outputBytes());
}

function query(url) {
  return encoder.encode(url.search.slice(1));
}

function render(url) {
  call(wasm.page, query(url));
  const doc = new DOMParser().parseFromString(output(), "text/html");
  document.title = doc.title;
  document.body.replaceWith(doc.body);
}

function download(url) {
  call(wasm.csv, query(url));
  const blob = new Blob([outputBytes()], { type: "text/csv;charset=utf-8" });
  wasm.csv_filename();
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = output();
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 60_000);
}

async function show(url) {
  document.documentElement.classList.add("loading");
  try {
    await ready;
    render(url);
  } finally {
    document.documentElement.classList.remove("loading");
  }
}

function navigate(url) {
  // Remember where this page was scrolled to, for the back button.
  history.replaceState({ scroll: scrollY }, "");
  history.pushState(null, "", url);
  show(url);
}

// Only the dashboard's own URLs; everything else (Job Bank, StatCan) navigates normally.
function ours(href) {
  const url = new URL(href, location.href);
  return url.origin === location.origin && (url.pathname === "/" || url.pathname === "/export.csv") ? url : null;
}

history.scrollRestoration = "manual";

document.addEventListener("click", (e) => {
  if (failed || e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  // Chart bars are SVG links, whose href and target aren't plain strings, so read the attributes.
  const a = e.target.closest("a[href]");
  if (!a || a.getAttribute("target")) return;
  const url = ours(a.getAttribute("href"));
  if (!url) return;
  e.preventDefault();
  if (url.pathname === "/export.csv") ready.then(() => download(url));
  else navigate(url);
});

document.addEventListener("submit", (e) => {
  const form = e.target;
  if (failed || form.method !== "get") return;
  const url = ours(form.getAttribute("action") || "/");
  if (!url || url.pathname !== "/") return;
  e.preventDefault();
  url.search = new URLSearchParams(new FormData(form)).toString();
  navigate(url);
});

addEventListener("popstate", async (e) => {
  await show(new URL(location.href));
  scrollTo(0, e.state?.scroll ?? 0);
});

// Every URL serves the default view, so draw the one this URL asks for. A direct link to the
// CSV starts the download and shows the matching view.
if (location.pathname === "/export.csv") {
  const url = new URL(location.href);
  history.replaceState(null, "", "/" + url.search);
  ready.then(() => download(url));
}
if (location.search) show(new URL(location.href));
