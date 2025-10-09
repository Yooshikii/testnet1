# Vecno WASM SDK

An integration wrapper around [`vecno-wasm`](https://www.npmjs.com/package/vecno-wasm) module that uses [`websocket`](https://www.npmjs.com/package/websocket) W3C adaptor for WebSocket communication.

This is a Node.js module that provides bindings to the VecnoWASM SDK strictly for use in the Node.js environment. The web browser version of the SDK is available as part of official SDK releases at [https://github.com/Vecno-Foundation/vecnod/releases](https://github.com/Vecno-Foundation/vecnod/releases)

## Usage

Vecno NPM module exports include all WASM32 bindings.

```javascript
const vecno = require('vecno');
console.log(vecno.version());
```

## Building from source & Examples

SDK examples as well as information on building the project from source can be found at [https://github.com/Vecno-Foundation/vecnod/releases/tree/master/wasm](https://github.com/Vecno-Foundation/vecnod/releases/tree/master/wasm)

## Releases

Official releases as well as releases for Web Browsers are available at [https://github.com/Vecno-Foundation/vecnod/releases](https://github.com/Vecno-Foundation/vecnod/releases).
