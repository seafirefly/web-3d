// sky pass

struct VertexOutput 
{
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@group(0) @binding(0) var<uniform>  inv_view_proj: mat4x4f;   // ViewProjection行列の逆行列
@group(0) @binding(1) var t_depth:  texture_depth_2d;         // 既存の深度バッファ
@group(0) @binding(2) var t_screen: texture_2d<f32>;		  // 既存のスクリーン
@group(0) @binding(3) var t_skybox: texture_cube<f32>;        // 変換済みのキューブマップ
@group(0) @binding(4) var s_skybox: sampler;                  // サンプラー

const PI: f32 = 3.14159265359;


@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput 
{
    // 画面全体を覆う三角形の頂点データ
    var pos = array<vec2f, 3>(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0)
    );
    var out: VertexOutput;
    let p = pos[vertex_index];
    out.position = vec4f(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y; // Y軸反転対応
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f 
{
	// スクリーンのピクセルを取得
	var final_color = textureLoad(t_screen, vec2u(in.position.xy), 0);

    // 現在のピクセルの深度を取得
    let depth = textureLoad(t_depth, vec2u(in.position.xy), 0);
	
    // NDC空間 (x: -1~1, y: -1~1, z: depth) からワールド空間の方向を復元
    let ndc = vec4f(
        in.uv.x * 2.0 - 1.0,
        (1.0 - in.uv.y) * 2.0 - 1.0,
        depth,
        1.0
    );

    let world_pos_h = inv_view_proj * ndc;
    let world_dir   = normalize(world_pos_h.xyz / world_pos_h.w);
	let sky_color   = textureSample(t_skybox, s_skybox, world_dir);
    
	return mix(final_color, sky_color, floor(depth));
}