// Classic SharedWorker entry. It must stay classic (`importScripts`, no ESM
// imports) because Dioxus preserves classic public workers but can rewrite
// module worker entry logic.
importScripts('./worker/address_explorer_web_worker.js');

const pendingPorts = [];
let wasm = null;

function connectPort(port) {
  if (wasm) {
    wasm.connect_port(port);
  } else {
    pendingPorts.push(port);
  }
}

self.onconnect = (event) => {
  for (const port of event.ports) {
    connectPort(port);
  }
};

wasm_bindgen('./worker/address_explorer_web_worker_bg.wasm')
  .then((initialized) => {
    wasm = initialized;
    for (const port of pendingPorts.splice(0)) {
      wasm.connect_port(port);
    }
  })
  .catch((error) => {
    console.error('address-explorer worker failed to start', error);
    for (const port of pendingPorts.splice(0)) {
      port.postMessage(
        JSON.stringify({
          id: 0,
          ok: false,
          error: 'the address-explorer worker could not initialize',
        }),
      );
    }
  });
