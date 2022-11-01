# zduny-wasm-timer

Exports the `Instant`, `Delay`, `Interval` and `Timeout` structs.

On non-WASM targets, this re-exports the types from `tokio-timer`.
On WASM targets, this uses `web-sys` and `js-sys` to implement their functionalities.

https://crates.io/crates/zduny-wasm-timer

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## see also
[wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)

[web-sys](https://rustwasm.github.io/wasm-bindgen/web-sys/index.html)

[js_sys](https://docs.rs/js-sys/latest/js_sys/)
