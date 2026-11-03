struct VertexOutput 
{
    @builtin(position) position   : vec4<f32>,
    @location(0)       uv         : vec2<f32>
};

struct BloomUniform
{
	threshold : f32
}

@group(0) @binding(0) var 		   t_diffuse     		    : texture_2d<f32>;
@group(0) @binding(1) var 		   s_diffuse     		    : sampler;
@group(0) @binding(2) var 		   t_bloom					: texture_2d<f32>;
@group(1) @binding(0) var<uniform> u_bloom					: BloomUniform;				


// 輝度計算（Rec.709）
fn luminance(v: vec3f) -> f32 
{
    return dot(v, vec3f(0.2126, 0.7152, 0.0722));
}

// Karis重み: 明るいピクセルの影響力を抑えて、縮小時のチラつきを防ぐ
fn karis_weight(v: vec3f) -> f32 
{
    let luma = luminance(v);
    return 1.0 / (1.0 + luma);
}


@vertex
fn vs_main( @builtin(vertex_index) vertex_index : u32 ) -> VertexOutput
{
    var out: VertexOutput;

    let x = f32(i32(vertex_index & 1u) << 2u) - 1.0;
    let y = f32(i32(vertex_index & 2u) << 1u) - 1.0;
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv       = vec2f(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    return out;
}


@fragment
fn fs_extraction_main(@location(0) uv: vec2f) -> @location(0) vec4f 
{
	let color = textureSample(t_diffuse, s_diffuse, uv).rgb;
    
    // 輝度の計算 (Rec.709 係数)
    let brightness = luminance(color);
    
    // 設定された閾値 (u_bloom.threshold) を超えた成分だけを抽出
    // 閾値以下を完全にカットすることで、光らせたい場所だけを制御できる
	let mask = step(u_bloom.threshold, brightness);
	return vec4f(color * mask, mask);
}


@fragment
fn fs_down_sampling_main(@location(0) uv: vec2f) -> @location(0) vec4f 
{
	let src_size = vec2f(textureDimensions(t_diffuse));
    let texel = 1.0 / src_size;
    let x = texel.x;
    let y = texel.y;

    // --- 13点のサンプリング ---
    // 中央とその周辺を網羅する配置
    let a = textureSample(t_diffuse, s_diffuse, uv + vec2f(-2.0*x, 2.0*y)).rgb;
    let b = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0,   2.0*y)).rgb;
    let c = textureSample(t_diffuse, s_diffuse, uv + vec2f( 2.0*x, 2.0*y)).rgb;

    let d = textureSample(t_diffuse, s_diffuse, uv + vec2f(-2.0*x, 0.0)).rgb;
    let e = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0,   0.0)).rgb; 
    let f = textureSample(t_diffuse, s_diffuse, uv + vec2f( 2.0*x, 0.0)).rgb;

    let g = textureSample(t_diffuse, s_diffuse, uv + vec2f(-2.0*x,-2.0*y)).rgb;
    let h = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0,  -2.0*y)).rgb;
    let i = textureSample(t_diffuse, s_diffuse, uv + vec2f( 2.0*x,-2.0*y)).rgb;

    let j = textureSample(t_diffuse, s_diffuse, uv + vec2f(-1.0*x, 1.0*y)).rgb;
    let k = textureSample(t_diffuse, s_diffuse, uv + vec2f( 1.0*x, 1.0*y)).rgb;
    let l = textureSample(t_diffuse, s_diffuse, uv + vec2f(-1.0*x,-1.0*y)).rgb;
    let m = textureSample(t_diffuse, s_diffuse, uv + vec2f( 1.0*x,-1.0*y)).rgb;

    // --- 4つのオーバーラップするグループ + 外側の平均 ---
    // これにより情報の取りこぼしを防ぐ
    let g1 = (a + b + d + e) * 0.25;
    let g2 = (b + c + e + f) * 0.25;
    let g3 = (d + e + g + h) * 0.25;
    let g4 = (e + f + h + i) * 0.25;
    let g5 = (j + k + l + m) * 0.25;

    // Karis重みを適用して重み付き平均をとる
    // (※Prefilterパス=1回目のダウンサンプル時のみKarisを適用し、以降は単純平均にすることもあります)
    let w1 = karis_weight(g1);
    let w2 = karis_weight(g2);
    let w3 = karis_weight(g3);
    let w4 = karis_weight(g4);
    let w5 = karis_weight(g5);

    let result = (g1 * w1 + g2 * w2 + g3 * w3 + g4 * w4 + g5 * w5) / (w1 + w2 + w3 + w4 + w5);

    return vec4f(result, 1.0);
}


@fragment
fn fs_up_sampling_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    // ぼかしの半径（フィルタの広がり）。通常は0.005前後
    // 解像度に合わせて調整可能ですが、固定値でも十分綺麗にボケます
    let d = 0.005;
    
    // 3x3のテントフィルターサンプリング
    // [a][b][c]
    // [d][e][f]
    // [g][h][i]
    let a = textureSample(t_diffuse, s_diffuse, uv + vec2f(-d,  d)).rgb;
    let b = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0, d)).rgb;
    let c = textureSample(t_diffuse, s_diffuse, uv + vec2f( d,  d)).rgb;

    let d_side = textureSample(t_diffuse, s_diffuse, uv + vec2f(-d,  0.0)).rgb;
    let e = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0, 0.0)).rgb; // 中心
    let f = textureSample(t_diffuse, s_diffuse, uv + vec2f( d,  0.0)).rgb;

    let g = textureSample(t_diffuse, s_diffuse, uv + vec2f(-d, -d)).rgb;
    let h = textureSample(t_diffuse, s_diffuse, uv + vec2f( 0.0,-d)).rgb;
    let i = textureSample(t_diffuse, s_diffuse, uv + vec2f( d, -d)).rgb;

    // テントフィルターの重み付け平均
    // 中心が最も強く、角が最も弱い
    let result = (a + c + g + i) * 1.0 + (b + d_side + f + h) * 2.0 + e * 4.0;
    
    return vec4f(result / 16.0, 1.0);
}


@fragment
fn fs_resolve_main(@location(0) uv: vec2f) -> @location(0) vec4f 
{
    let hdr_color   = textureSample(t_diffuse, s_diffuse, uv).rgb;
    let bloom_color = textureSample(t_bloom, s_diffuse, uv).rgb;
    
    // 1. 加算合成 (強度 0.4 で調整)
    var result = hdr_color + (bloom_color * 0.4);
    
    return vec4f(result, 1.0);
}