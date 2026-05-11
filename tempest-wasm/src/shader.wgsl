struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen triangle
    let x = f32(i32(in_vertex_index & 1u) * 4 - 1);
    let y = f32(i32(in_vertex_index & 2u) * 2 - 1);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    out.color = vec3<f32>(0.0, 0.5, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let dist = length(uv);
    
    // 🌪️ Vortex Swirl Calculation
    let angle = atan2(uv.y, uv.x);
    let swirl = angle + (2.0 / (dist + 0.1));
    
    // Dynamic Colors (Neon Cyan & Vortex Purple)
    let cyan = vec3<f32>(0.0, 0.95, 1.0);
    let purple = vec3<f32>(0.44, 0.0, 1.0);
    let deep_space = vec3<f32>(0.02, 0.02, 0.04);
    
    // Ring patterns
    let rings = sin(swirl * 8.0 - dist * 4.0) * 0.5 + 0.5;
    let glow = 0.05 / (dist + 0.05);
    
    let mixed_color = mix(cyan, purple, sin(dist * 2.0) * 0.5 + 0.5);
    let final_color = mix(deep_space, mixed_color, rings * glow);
    
    return vec4<f32>(final_color + (mixed_color * glow * 0.2), 1.0);
}
