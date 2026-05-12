# Plan: Hardware rendering on V3D 4.2 without vertex-stage SSBOs

## Goal

Enable Zed to run with hardware rendering on Raspberry Pi 400 (VideoCore VI / V3D 4.2)
instead of falling through to llvmpipe.

## Constraint

V3D 4.2 supports `max_vertex_shader_storage_blocks = 0`. Every vertex shader in
`shaders.wgsl` reads per-instance data from a storage buffer, so all hardware adapters
(V3D Vulkan and V3D GL) are currently rejected by the vertex SSBO smoke test in
`try_adapter_with_surface`.

Reference: `ISSUE-wgpu-hal-vulkan-feature-not-present-panics.md`

---

## Phase 0 — Probe fragment SSBO support on V3D (prerequisite)

Before implementing anything, determine whether V3D allows storage buffers in the
fragment stage.

Evidence suggests it does:
- `vulkaninfo` reports `maxPerStageDescriptorStorageBuffers = 8` with no per-stage
  breakdown. The vertex limitation comes from Mesa NIR's V3D backend, not from the
  Vulkan property.
- The system's OpenGL extensions include `GL_ARB_shader_storage_buffer_object`, which
  typically implies fragment SSBO support even when vertex SSBO support is absent.

**How to verify**: Add a second smoke test to `try_adapter_with_surface` that creates
a pipeline using a storage buffer bound with `ShaderStages::FRAGMENT` only (no vertex
access). If it passes, Approach A is viable. If it fails, fall back to Approach B.

This probe can reuse the same smoke-test pattern already in the file:
wrap a `create_render_pipeline` call in an `ErrorFilter::Internal` scope; treat failure
as `fragment_storage = false`.

Store the result on `WgpuContext` as a `GpuCapabilities` struct:

```rust
pub struct GpuCapabilities {
    pub vertex_storage: bool,
    pub fragment_storage: bool,
    pub dual_source_blending: bool,
}
```

---

## Approach A — Split SSBO visibility, vertex data via VBO (preferred)

Applies when: `fragment_storage = true`, `vertex_storage = false`.

### Key insight

The instance buffer is already one contiguous allocation. In the split approach it is
bound in two roles simultaneously:
- As a **storage buffer** with `FRAGMENT`-only visibility — fragment shaders continue to
  read `b_quads[input.quad_id]` as today.
- As a **vertex buffer** (`PerInstance` step mode) — vertex shaders receive per-instance
  fields as `@location(N)` attributes instead of indexing the SSBO.

`wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE` is valid. No data duplication
is needed.

### What changes

#### 1. Capability detection in `WgpuContext`

- Replace the hard adapter-rejection smoke test with a soft capability probe.
- `try_adapter_with_surface` now produces `GpuCapabilities` instead of `bool`.
- An adapter is only rejected if it fails the surface configuration test. Lacking vertex
  SSBO support is a capability difference, not a failure.

#### 2. Bind group layout (`wgpu_renderer.rs`)

- `storage_buffer_entry` accepts a `visibility` parameter.
- When `vertex_storage = false`, pass `ShaderStages::FRAGMENT` for instance data
  bind group layouts.
- Globals bind group (`ShaderStages::VERTEX_FRAGMENT`) is unchanged.

#### 3. Instance buffer usage flags (`wgpu_renderer.rs`)

- When `vertex_storage = false`, create the instance buffer with:
  `wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST`

#### 4. Pipeline creation (`wgpu_renderer.rs`)

- When `vertex_storage = false`, each pipeline is created with a
  `wgpu::VertexBufferLayout` describing the per-instance struct, using
  `wgpu::VertexStepMode::Instance` and `array_stride = mem::size_of::<T>()`.
- The `draw_primitives` helper calls `render_pass.set_vertex_buffer(1, ...)` instead
  of relying solely on the bind group.

#### 5. Shader changes — vertex entry points only

Add a `_vbo` variant of each vertex entry point. The fragment entry points are
unchanged. Both live in the same `shaders.wgsl` file; the pipeline is constructed with
`entry_point: Some("vs_quad_vbo")` when in VBO mode.

Example for `vs_quad`:

```wgsl
// Existing SSBO path (unchanged):
@vertex fn vs_quad(
    @builtin(vertex_index) vertex_id: u32,
    @builtin(instance_index) instance_id: u32,
) -> QuadVarying {
    let quad = b_quads[instance_id];
    ...
}

// New VBO path (vertex_storage = false):
struct QuadInstance {
    @location(0) bounds:          vec4<f32>,   // origin.xy, size.xy
    @location(1) content_mask:    vec4<f32>,   // origin.xy, size.xy
    @location(2) bg_flags:        vec2<u32>,   // tag, color_space
    @location(3) bg_angle_pad:    vec2<f32>,   // gradient_angle, _pad
    @location(4) bg_solid:        vec4<f32>,   // Hsla
    @location(5) bg_stop0_color:  vec4<f32>,   // LinearColorStop[0].color Hsla
    @location(6) bg_stop0_pct:    f32,          // LinearColorStop[0].percentage
    @location(7) bg_stop1_color:  vec4<f32>,   // LinearColorStop[1].color Hsla
    @location(8) bg_stop1_pct:    f32,          // LinearColorStop[1].percentage
    @location(9) border_color:    vec4<f32>,   // Hsla (pre-converted for vertex)
    @location(10) quad_id_inst:   u32,          // instance index for fragment re-read
}

@vertex fn vs_quad_vbo(
    @builtin(vertex_index) vertex_id: u32,
    instance: QuadInstance,
) -> QuadVarying {
    // reconstruct Bounds/Background from instance attributes and call same helpers
    ...
}
```

The fragment shader `fs_quad` is unchanged — it still reads `b_quads[input.quad_id]`
from the fragment-only SSBO binding.

### Per-pipeline vertex attribute summary

| Pipeline | Struct | Bytes/instance | Approx. locations |
|---|---|---|---|
| `quads` | `Quad` | ~160 | 11 |
| `shadows` | `Shadow` | ~64 | 5 |
| `underlines` | `Underline` | ~32 | 3 |
| `path_rasterization` | `PathRasterizationVertex` | ~32 | 3 |
| `path_sprites` | `PathSprite` | ~80 | 6 |
| `mono_sprites` | `MonochromeSprite` | ~80 | 6 |
| `poly_sprites` | `PolychromeSprite` | ~96 | 7 |
| `subpixel_sprites` | `SubpixelSprite` | ~80 | 6 |

All are within the WebGPU minimum of 16 vertex attribute locations.

### Alignment caution

The WGSL `@location(N)` attribute format must match the Rust struct field layout
byte-for-byte. Use `#[repr(C)]` on all primitive structs and verify offsets against
WGSL's own struct alignment rules (WGSL aligns each member to its own alignment, which
may differ from `repr(C)`). A mismatch produces corrupted per-instance reads without
any compile-time error.

---

## Approach B — Full VBO, no SSBOs in any stage

Applies when: both `vertex_storage = false` and `fragment_storage = false`.

Fragment shaders no longer have access to the SSBO. All data the fragment shader needs
must arrive as `@interpolate(flat)` varyings from the vertex shader.

### Varying size estimate for `Quad`

The fragment shader needs: `bounds` (vec4f), `corner_radii` (vec4f), `border_widths`
(vec4f), `background` (tag + color_space + angle + solid + 2 stops ≈ 6 locations),
`border_color` (already in varying). Total: ~12 locations added to `QuadVarying`.
Still within the 16-location limit.

### When to do this

Only if Phase 0 confirms `fragment_storage = false` on V3D. Given the Mesa extension
presence and Vulkan limits, this is unlikely. Implement Approach A first; add Approach B
only if B is needed for a specific target.

---

## Implementation order

1. **Phase 0 probe**: Add `GpuCapabilities` struct and the fragment SSBO probe to
   `try_adapter_with_surface`. Verify on hardware that V3D gets
   `fragment_storage = true`. (~50 lines in `wgpu_context.rs`)

2. **Capability propagation**: Thread `GpuCapabilities` through `WgpuContext` and into
   `WgpuRenderer`. No functional change yet.

3. **Buffer + bind group split**: When `vertex_storage = false`, add `VERTEX` usage to
   the instance buffer and change the storage binding visibility to `FRAGMENT`.
   (~20 lines in `wgpu_renderer.rs`)

4. **Pipeline vertex layouts**: For each pipeline, define the `VertexBufferLayout` in
   Rust and call `set_vertex_buffer` in the draw path. (~200 lines in
   `wgpu_renderer.rs`)

5. **Shader VBO variants**: Add `_vbo` vertex entry points for each pipeline that map
   the vertex attributes back to the struct fields and call the existing helper
   functions. Pick the entry point at pipeline creation time.
   (~400 lines in `shaders.wgsl`, `shaders_subpixel.wgsl`)

6. **Remove smoke-test rejection**: Change `try_adapter_with_surface` to no longer
   bail on `vertex_storage = false`; only bail on surface config failure.

7. **Validate on Pi 400**: Build, run `zed .`, confirm V3D GL (or Vulkan) is selected
   and no rendering artifacts appear.

---

## Out of scope

- Removing SSBOs from the desktop (Vulkan/Metal/DX12) path — no benefit, only
  churn for hardware that supports vertex SSBOs fine.
- WASM/browser path.
- Performance tuning; correctness comes first.

---

## Key files

| File | Change |
|---|---|
| `crates/gpui_wgpu/src/wgpu_context.rs` | Add `GpuCapabilities`, probes, expose capability |
| `crates/gpui_wgpu/src/wgpu_renderer.rs` | Conditional buffer usage, bind group layout, vertex buffer setup |
| `crates/gpui_wgpu/src/shaders.wgsl` | `_vbo` vertex entry points for all 7 pipelines |
| `crates/gpui_wgpu/src/shaders_subpixel.wgsl` | `_vbo` vertex entry point for subpixel |
