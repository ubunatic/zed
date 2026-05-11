# wgpu-hal / wgpu: V3D Vulkan crashes instead of falling back on Raspberry Pi

## Summary

Two separate bugs in the wgpu stack cause crashes on V3D 4.2 (Raspberry Pi 400) instead of
gracefully falling back to the next adapter (llvmpipe Vulkan or GL). Both bugs were hit in
sequence during debugging; both are upstream issues.

---

## Bug 1 — `VK_ERROR_FEATURE_NOT_PRESENT` panics instead of returning `DeviceError`

### Affected code

`wgpu-hal/src/vulkan/adapter.rs` — `map_err` inside `Adapter::open_with_callback`:

```rust
fn map_err(err: vk::Result) -> crate::DeviceError {
    match err {
        vk::Result::ERROR_TOO_MANY_OBJECTS => crate::DeviceError::OutOfMemory,
        vk::Result::ERROR_INITIALIZATION_FAILED => crate::DeviceError::Lost,
        vk::Result::ERROR_EXTENSION_NOT_PRESENT | vk::Result::ERROR_FEATURE_NOT_PRESENT => {
            crate::hal_usage_error(err)   // <-- panics
        }
        other => super::map_host_device_oom_and_lost_err(other),
    }
}
```

### Reproduction

Hardware: Raspberry Pi 400 (VideoCore VI / V3D 4.2.14.0, aarch64)  
Driver: Mesa V3D Vulkan (Vulkan 1.1, `dualSrcBlend = false`)

1. Build any wgpu app with `panic = "abort"` that requests a Vulkan device.
2. `vkCreateDevice` returns `VK_ERROR_FEATURE_NOT_PRESENT`.
3. `hal_usage_error` fires → process aborts immediately.

```
wgpu-hal invariant was violated (usage error): Requested feature is not available on this device
wgpu_hal::hal_usage_error
wgpu_hal::vulkan::adapter::<impl wgpu_hal::vulkan::Adapter>::open_with_callback
wgpu_core::instance::Adapter::create_device_and_queue
wgpu_core::instance::<impl wgpu_core::global::Global>::adapter_request_device
wgpu::api::adapter::Adapter::request_device
```

`VK_ERROR_FEATURE_NOT_PRESENT` from `vkCreateDevice` is a driver-level runtime rejection,
not a caller bug. `hal_usage_error` is intended for wgpu API misuse (e.g. using a destroyed
resource). Using it here prevents any recovery, especially with `panic = "abort"` where
`catch_unwind` is a no-op.

The V3D driver returns this even though `adapter.features()` did not flag the feature as
unavailable — indicating a mismatch between what wgpu enables in `PhysicalDeviceFeatures`
and what V3D actually supports at the Vulkan layer.

### Proposed fix

```rust
vk::Result::ERROR_EXTENSION_NOT_PRESENT | vk::Result::ERROR_FEATURE_NOT_PRESENT => {
    crate::DeviceError::Unexpected
}
```

Propagates as a normal `Err` so callers can skip this adapter and try the next one.

---

## Bug 2 — `VERTEX_STORAGE` unconditionally set for Vulkan; pipeline failure panics

### Affected code

**wgpu-hal/src/vulkan/adapter.rs** — downlevel flags setup:

```rust
let mut dl_flags = Df::COMPUTE_SHADERS
    | Df::BASE_VERTEX
    | ...
    | Df::VERTEX_STORAGE    // <-- unconditionally set for ALL Vulkan adapters
    | Df::FRAGMENT_STORAGE
    | ...
```

**wgpu/src/backend/wgpu_core.rs:1409** — default error handler:

```rust
fn default_error_handler(err: crate::Error) -> ! {
    log::error!("Handling wgpu errors as fatal by default");
    panic!("wgpu error: {err}\n");
}
```

### Reproduction

After Bug 1 is fixed, V3D Vulkan successfully creates a device but crashes at pipeline
creation:

```
wgpu error: Validation Error
Caused by:
  In Device::create_render_pipeline, label = 'quads'
    Internal error in ShaderStages(VERTEX | FRAGMENT) shader:
    error: Too many vertex shader storage blocks (1/0)

wgpu::backend::wgpu_core::default_error_handler
wgpu_core::instance::<impl ...>::create_render_pipeline
```

V3D 4.2 supports 0 vertex-stage SSBOs. Mesa's GLSL/NIR compiler enforces this at
pipeline-creation time. However, wgpu-hal unconditionally marks `VERTEX_STORAGE` as
available for all Vulkan adapters, so the adapter passes capability checks and gets
selected. The error is reported as `ErrorType::Internal` (via
`CreateRenderPipelineError::Internal`) and hits `default_error_handler`, which panics.

Two distinct upstream problems:
1. `VERTEX_STORAGE` is set unconditionally for Vulkan regardless of whether the driver
   actually supports SSBOs in vertex shaders.
2. `default_error_handler` panics on `Internal` errors, even ones that could be handled
   (e.g. by an active `push_error_scope(ErrorFilter::Internal)`).

### Proposed fix (wgpu-hal)

Gate `VERTEX_STORAGE` on whether the device actually supports it, e.g. by checking
`max_per_stage_descriptor_storage_buffers` or attempting a probe at adapter enumeration.

### Proposed fix (wgpu)

Change `default_error_handler` to not panic, or document clearly that callers must
always use error scopes to avoid hitting it. A non-panicking default would allow
graceful degradation:

```rust
fn default_error_handler(err: crate::Error) {  // remove `-> !`
    log::error!("wgpu uncaptured error: {err}");
}
```

---

## Workaround applied in Zed (gpui_wgpu)

Since both upstream bugs affect `panic = "abort"` release builds with no recovery path,
a smoke-test was added to `WgpuContext::try_adapter_with_surface` that creates a minimal
render pipeline with a vertex storage buffer binding, wrapped in `Internal` + `Validation`
error scopes. If the test fails, the adapter is rejected and the loop tries the next one
(llvmpipe Vulkan or GL).

This avoids the panic in both bugs: Bug 1 is caught because `DeviceError::Unexpected`
propagates as `Err`; Bug 2 is caught because the pipeline error is inside an error scope
rather than hitting `default_error_handler`.

## Build notes: applying patches to the Cargo git cache

The wgpu fixes live in the Cargo git checkout, not in the Zed tree:

```
~/.cargo/git/checkouts/wgpu-423de87c978aca7f/a466bc3/
```

Cargo fingerprints git dependencies by **HEAD commit hash**, not file mtime. Editing a
file in the checkout does not invalidate the fingerprint, so Cargo silently reuses the
old compiled artifact. After any edit to cached wgpu sources, delete the fingerprint for
the affected crate before building:

```sh
# release-min
rm -rf target/release-min/.fingerprint/wgpu-hal-*
cargo build --profile release-min -j3

# dev / example
rm -rf target/debug/.fingerprint/wgpu-hal-*
cargo build -p gpui --example hello_world -j4
```

If `wgpu-core` or `wgpu` are also patched, add their fingerprints to the `rm` line.

### Proper long-term path

Commit the fix to the upstream fork and update the branch pointer in `Cargo.toml`:

```toml
wgpu = { git = "https://github.com/zed-industries/wgpu.git", branch = "fix/v3d-fallback" }
```

Then `cargo update -p wgpu` to refresh the lockfile. No fingerprint surgery needed.

---

## Hardware / software context

- Device: Raspberry Pi 400
- GPU: VideoCore VI (V3D 4.2.14.0), aarch64
- OS: Debian Linux 6.12, Mesa V3D Vulkan 1.1
- `vulkaninfo` reports: `dualSrcBlend = false` for V3D
- wgpu fork: `zed-industries/wgpu`, branch `v29`
