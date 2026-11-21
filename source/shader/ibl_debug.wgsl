struct DebugParams {
    use_prefilter : f32,
    lod : f32,
    _pad : vec2<f32>,
}

@group(0) @binding(0) var t_irradiance : texture_cube<f32>;
@group(0) @binding(1) var t_prefilter : texture_cube<f32>;
@group(0) @binding(2) var s_env : sampler;
@group(0) @binding(3) var<uniform> u_debug : DebugParams;

struct VertexOutput {
    @builtin(position) position : vec4<f32>,
    @location(0) uv : vec2<f32>,
}

// Fullscreen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index : u32) -> VertexOutput {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    var out : VertexOutput;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// Draws the cube map as a cross layout (4 x 3 cells):
//        [+Y]
//  [-X]  [+Z]  [+X]  [-Z]
//        [-Y]
@fragment
fn fs_main(in : VertexOutput) -> @location(0) vec4<f32> {
    let grid = vec2<f32>(in.uv.x * 4.0, in.uv.y * 3.0);
    let cell = floor(grid);
    let local = fract(grid) * 2.0 - vec2<f32>(1.0, 1.0);
    let u = local.x;
    let v = local.y;

    var dir = vec3<f32>(0.0, 0.0, 0.0);
    var is_face = true;
    if (cell.x == 2.0 && cell.y == 1.0) {
        dir = vec3<f32>(1.0, -v, -u);
    } else if (cell.x == 0.0 && cell.y == 1.0) {
        dir = vec3<f32>(-1.0, -v, u);
    } else if (cell.x == 1.0 && cell.y == 0.0) {
        dir = vec3<f32>(u, 1.0, v);
    } else if (cell.x == 1.0 && cell.y == 2.0) {
        dir = vec3<f32>(u, -1.0, -v);
    } else if (cell.x == 1.0 && cell.y == 1.0) {
        dir = vec3<f32>(u, -v, 1.0);
    } else if (cell.x == 3.0 && cell.y == 1.0) {
        dir = vec3<f32>(-u, -v, -1.0);
    } else {
        is_face = false;
    }

    if (!is_face) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let n = normalize(dir);
    var color = textureSampleLevel(t_irradiance, s_env, n, 0.0).rgb;
    if (u_debug.use_prefilter > 0.5) {
        color = textureSampleLevel(t_prefilter, s_env, n, u_debug.lod).rgb;
    }
    return vec4<f32>(color, 1.0);
}
