struct VertexOutput {
    @builtin(position) position   : vec4<f32>,
    @location(0)       uv         : vec2<f32>
};

struct Uniform 
{
    inv_projection_matrix  : mat4x4<f32>,
    inv_view_matrix        : mat4x4<f32>,
    camera_position        : vec4<f32>,
    grid_spacing           : f32,
    line_thickness         : f32,
    fade_radius            : f32,
    _padding               : f32,
}

@group(0) @binding(0) var<uniform> in_uniform         : Uniform;
@group(1) @binding(0) var          depth_texture      : texture_depth_2d;

@vertex
fn vs_main( @builtin(vertex_index) vertex_index : u32 ) -> VertexOutput
{
    var out: VertexOutput;

    let x = f32(i32(vertex_index & 1u) << 2u) - 1.0;
    let y = f32(i32(vertex_index & 2u) << 1u) - 1.0;
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv       = vec2<f32>(x * 0.5 + 0.5, (y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> 
{
    let texel_coord = vec2<i32>(in.position.xy);
    let depth       = textureLoad(depth_texture, texel_coord, 0);

    let ndc          = vec4f(in.uv * 2.0 - 1.0, depth, 1.0);
    let view_target  = in_uniform.inv_view_matrix * in_uniform.inv_projection_matrix * ndc;
    let ray_dir      = normalize(view_target.xyz / view_target.w - in_uniform.camera_position.xyz);

    // 2. レイとXY平面 (z=0) の交差判定
    // 式: camera_pos.z + t * ray_dir.z = 0
    let t = -in_uniform.camera_position.z / ray_dir.z;

    // 平面がカメラの後ろにある場合、または平行な場合は描画しない
    if (t < 0.0) { discard; }
    // 深度判定は少しだけファジーにして、z=0 付近の平面でもグリッドを表示できるようにする
    let depth_epsilon = 1e-3;
    if (abs(view_target.z) > depth_epsilon && in_uniform.camera_position.z * view_target.z > 0.0) { discard; }

    let world_pos = in_uniform.camera_position.xyz + ray_dir * t;
    let coord     = world_pos.xy;

    // 3. グリッドの描画 (1m単位)
    // fwidth() で画面上の 1px あたりのワールド単位を取得し、
    // カメラ距離に関わらず見た目の太さを一定にする。
    let grid_size  = 1.0;
    let line_width_pixels = in_uniform.line_thickness;
    let coord_step = max(fwidth(coord.x), fwidth(coord.y));
    let line_width = max(coord_step * line_width_pixels, 1e-5);
    let grid = abs(fract(coord / grid_size - 0.5) - 0.5) / (line_width * 0.5);
    var line = min(grid.x, grid.y);
    
    // 線の鋭さを滑らかにする
    let color_mask = 1.0 - floor(smoothstep(0.0, 1.0, line));

    // 4. 軸の色付け (X軸=赤, Y軸=緑)
    var grid_color = vec3f(0.3); // 通常の線はグレー
    if (abs(coord.y) < line_width) { grid_color = vec3f(1.0, 0.0, 0.0); } // X軸
    if (abs(coord.x) < line_width) { grid_color = vec3f(0.0, 1.0, 0.0); } // Y軸

    // 距離に応じてフェードアウト（遠くのノイズを防ぐ）
    let opacity = color_mask * exp(-0.15 * t);

    if (opacity < 0.01) { discard; }
    return vec4f(grid_color, opacity);
}