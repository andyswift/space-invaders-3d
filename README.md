# Space Invaders 3D: The Upside Down Odyssey

Rust + [Bevy](https://bevyengine.org) 0.18, compiled to WebAssembly. Live at <https://moaswift.no/invaders-3d/>.

Every ~30 s the ground rumbles and the world rolls over into a cave: new environment, new 3D creatures,
a countdown back to the surface. The ship, shots and controls never change.

## Develop

```sh
brew install trunk            # build tool (downloads wasm-bindgen itself)
trunk serve --port 8081       # WebGL2 build, live reload
trunk serve --port 8082 --no-default-features --features webgpu   # WebGPU build (adds bloom)
```

Debug builds only: press **F** to bring the next world flip forward.

## Why two builds?

Bevy ships either WebGL2 or WebGPU in one wasm, not both, and bloom needs WebGPU. So `loader/index.html`
sends browsers with a working WebGPU adapter to `gpu/` and everyone else to `gl/` (`?gl` forces WebGL).

## Release and deploy

```sh
trunk build --release --public-url ./ --dist dist-release/gpu --no-default-features --features webgpu
trunk build --release --public-url ./ --dist dist-release/gl
brew install brotli
AWS_PROFILE=moaswift ./deploy.sh --dry-run
AWS_PROFILE=moaswift ./deploy.sh
```

`deploy.sh` Brotli-compresses the wasm (about 4.3 MB per visitor), uploads to `s3://moaswift/invaders-3d/`
with the right headers, and clears that path in CloudFront. The main site's deploy leaves this folder alone.

## Layout

- `src/data.rs`: pixel art and the voxel mesh builder
- `src/game.rs`: rules, 3D table, projectiles, bunkers, responsive camera fit (fixed 60 Hz)
- `src/realm.rs`: the two worlds, procedural creatures, the flip
- `src/ui.rs`: HUD and messages
- `index.html`: page shell (full screen, safe areas, sound synth, fullscreen button)
