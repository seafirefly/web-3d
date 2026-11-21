## Requirements

- Rust (stable, edition 2021)
- Target: `wasm32-unknown-unknown`
- Build tool: [Trunk](https://trunkrs.dev/)
- Runtime: Since wgpu is used, a modern browser with WebGPU support (e.g. Chrome or Edge) is recommended

Setup example:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk build   # build
trunk serve   # start a local server
```

> Targets other than `wasm32-unknown-unknown` are not supported.

## References

Resources used in this project:

### Code and Examples

- [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples/src) - Reference implementations for rendering with wgpu
- [Trunk wasm_threads example](https://github.com/trunk-rs/trunk/tree/main/examples/wasm_threads) - Building WebAssembly projects with Trunk
- [lil-gui Kitchen Sink](https://lil-gui.georgealways.com/examples/kitchen-sink/) - Usage examples for the lil-gui controls
- [Rust/wgpuをWebブラウザで動かす (Zenn)](https://zenn.dev/matcha_choco010/articles/2022-07-11-wgpu-web) - Running wgpu in the browser

### Assets and Data

- [McGuire Computer Graphics Archive](https://casual-effects.com/data/) - Test models for computer graphics
- [Sketchfab](https://sketchfab.com/) - 3D models
- [Poly Haven](https://polyhaven.com/) - 3D models, HDRIs, and textures (CC0)

Assets from Poly Haven are released under CC0. Sketchfab models have individual licenses, so please check the license on each model's page.
