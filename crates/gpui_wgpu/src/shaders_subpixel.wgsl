// --- subpixel sprites --- //

struct SubpixelSprite {
    order: u32,
    pad: u32,
    bounds: Bounds,
    content_mask: Bounds,
    color: Hsla,
    tile: AtlasTile,
    transformation: TransformationMatrix,
}
@group(1) @binding(0) var<storage, read> b_subpixel_sprites: array<SubpixelSprite>;

struct SubpixelSpriteOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(3) clip_distances: vec4<f32>,
}

struct SubpixelSpriteFragmentOutput {
    @location(0) @blend_src(0) foreground: vec4<f32>,
    @location(0) @blend_src(1) alpha: vec4<f32>,
}

@vertex
fn vs_subpixel_sprite(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> SubpixelSpriteOutput {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
    let sprite = b_subpixel_sprites[instance_id];

    var out = SubpixelSpriteOutput();
    out.position = to_device_position_transformed(unit_vertex, sprite.bounds, sprite.transformation);
    out.tile_position = to_tile_position(unit_vertex, sprite.tile);
    out.color = hsla_to_rgba(sprite.color);
    out.clip_distances = distance_from_clip_rect_transformed(unit_vertex, sprite.bounds, sprite.content_mask, sprite.transformation);
    return out;
}

// --- subpixel sprites (VBO) ---
// SubpixelSprite stride 112, step Instance: same field layout as MonochromeSprite.
// Outputs SubpixelSpriteOutput; fs_subpixel_sprite is reused unchanged.

struct SubpixelSpriteInstanceVbo {
    @location(0) bounds:         vec4<f32>,
    @location(1) content_mask:   vec4<f32>,
    @location(2) color:          vec4<f32>,
    @location(3) tile_bounds:    vec4<i32>,
    @location(4) rotation_scale: vec4<f32>,
    @location(5) translation:    vec2<f32>,
}

@vertex
fn vs_subpixel_sprite_vbo(
    @builtin(vertex_index)   vertex_id:   u32,
    @builtin(instance_index) instance_id: u32,
    inst: SubpixelSpriteInstanceVbo,
) -> SubpixelSpriteOutput {
    let unit_vertex  = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
    let bounds       = Bounds(inst.bounds.xy, inst.bounds.zw);
    let content_mask = Bounds(inst.content_mask.xy, inst.content_mask.zw);

    let position = unit_vertex * bounds.size + bounds.origin;
    let rs = inst.rotation_scale;
    let transformed = vec2<f32>(
        rs.x * position.x + rs.y * position.y,
        rs.z * position.x + rs.w * position.y,
    ) + inst.translation;

    let atlas_size    = vec2<f32>(textureDimensions(t_sprite, 0));
    let tile_position = (vec2<f32>(inst.tile_bounds.xy) +
                         unit_vertex * vec2<f32>(inst.tile_bounds.zw)) / atlas_size;

    var out = SubpixelSpriteOutput();
    out.position       = to_device_position_impl(transformed);
    out.tile_position  = tile_position;
    out.color          = hsla_to_rgba(Hsla(inst.color.x, inst.color.y, inst.color.z, inst.color.w));
    out.clip_distances = distance_from_clip_rect_impl(transformed, content_mask);
    return out;
}

@fragment
fn fs_subpixel_sprite(input: SubpixelSpriteOutput) -> SubpixelSpriteFragmentOutput {
    var sample = textureSample(t_sprite, s_sprite, input.tile_position).rgb;
    if (gamma_params.is_bgr != 0u) {
        sample = sample.bgr;
    }
    let alpha_corrected = apply_contrast_and_gamma_correction3(sample, input.color.rgb, gamma_params.subpixel_enhanced_contrast, gamma_params.gamma_ratios);

    // Alpha clip after using the derivatives.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return SubpixelSpriteFragmentOutput(vec4<f32>(0.0), vec4<f32>(0.0));
    }

    var out = SubpixelSpriteFragmentOutput();
    out.foreground = vec4<f32>(input.color.rgb, 1.0);
    out.alpha = vec4<f32>(input.color.a * alpha_corrected, 1.0);
    return out;
}
