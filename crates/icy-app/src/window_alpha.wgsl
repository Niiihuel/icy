@group(0) @binding(0) var scene: texture_2d<f32>;
override straight_alpha: bool = false;

@vertex fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let corner = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment fn fragment(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let pixel = textureLoad(scene, vec2i(position.xy), 0);
    let alpha = clamp(pixel.a - 1.0, 0.0, 1.0);
    let rgb = clamp(pixel.rgb, vec3f(0.0), vec3f(alpha));
    if straight_alpha && alpha > 0.0 {
        return vec4f(rgb / alpha, alpha);
    }
    return vec4f(rgb, alpha);
}
