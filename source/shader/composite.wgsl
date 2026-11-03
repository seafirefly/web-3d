struct VertexOutput 
{
    @builtin(position) position   : vec4<f32>,
    @location(0)       uv         : vec2<f32>
};

struct CompositeUniform
{
    exposure                : f32,
    saturation              : f32,
    tone_mapping_mode       : f32,
    highlight_rolloff       : f32,
    white_point             : f32,
    view_transform          : f32,
    use_dither              : f32,
    is_use_gamma_correction : f32,
    _padding0               : f32,
    _padding1               : f32,
}

@group(0) @binding(0) var           t_scene      : texture_2d<f32>;
@group(0) @binding(1) var           s_scene      : sampler;
@group(1) @binding(0) var<uniform>  u_composite  : CompositeUniform;

fn tonemap_aces(x: vec3f) -> vec3f 
{
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3f(0.0), vec3f(1.0));
}

fn tonemap_filmic(x: vec3f) -> vec3f {
    // Hable / Uncharted2 filmic curve
    let a = 0.15;
    let b = 0.50;
    let c = 0.10;
    let d = 0.20;
    let e = 0.02;
    let f = 0.30;
    let w = 11.2;

    let mapped = ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f;
    let white = ((vec3f(w) * (a * vec3f(w) + c * b) + d * e)
        / (vec3f(w) * (a * vec3f(w) + b) + d * f))
        - e / f;

    return clamp(mapped / white, vec3f(0.0), vec3f(1.0));
}

fn agx_default_contrast_approx(x: vec3f) -> vec3f {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2
        - 40.14 * x4 * x
        + 31.96 * x4
        - 6.868 * x2 * x
        + 0.4298 * x2
        + 0.1191 * x
        - 0.00232;
}

fn tonemap_agx(color: vec3f) -> vec3f {
    // Blender AgX-ish approximation: inset matrix -> log2 shaping -> contrast curve -> outset matrix
    let inset = mat3x3<f32>(
        vec3f(0.842479062253094, 0.0784335999999992, 0.0792237451477643),
        vec3f(0.0423282422610123, 0.878468636469772, 0.0791661274605434),
        vec3f(0.0423756549057051, 0.0784336, 0.879142973793104)
    );

    let outset = mat3x3<f32>(
        vec3f(1.19687900512017, -0.0980208811401368, -0.0990297440797205),
        vec3f(-0.0528968517574562, 1.15190312990417, -0.0989611768448433),
        vec3f(-0.0529716355144438, -0.0980434501171241, 1.15107367264116)
    );

    let min_ev = -12.47393;
    let max_ev = 4.026069;

    var v = inset * max(color, vec3f(1e-6));
    v = clamp((log2(v) - min_ev) / (max_ev - min_ev), vec3f(0.0), vec3f(1.0));
    v = agx_default_contrast_approx(v);
    return clamp(outset * v, vec3f(0.0), vec3f(1.0));
}

fn gamma_encode(x: vec3f) -> vec3f {
    let c = max(x, vec3f(0.0));

    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055;
    let encoded = select(lo, hi, c > vec3f(0.0031308));

    return clamp(encoded, vec3f(0.0), vec3f(1.0));
}

fn apply_exposure(color: vec3f, exposure: f32) -> vec3f {
    return color * exp2(exposure);
}

fn apply_saturation(color: vec3f, saturation: f32) -> vec3f {
    let luma = dot(color, vec3f(0.2126, 0.7152, 0.0722));
    return mix(vec3f(luma), color, saturation);
}

fn apply_highlight_rolloff(color: vec3f, amount: f32) -> vec3f {
    let x = max(color, vec3f(0.0));
    let shoulder = x / (x + vec3f(1.0));
    return mix(x, shoulder, amount);
}

fn apply_white_point(color: vec3f, white_point: f32) -> vec3f {
    let wp = max(white_point, 0.001);
    return color / vec3f(wp);
}

fn apply_tone_map(color: vec3f, mode: f32) -> vec3f {
    if (mode < 0.5) {
        return color;
    }
    if (mode < 1.5) {
        return tonemap_aces(color);
    }
    if (mode < 2.5) {
        return tonemap_filmic(color);
    }
    return tonemap_agx(color);
}

fn apply_view_transform(color: vec3f, mode: f32) -> vec3f {
    if (mode < 0.5) {
        return color;
    }
    if (mode < 1.5) {
        return color * vec3f(0.95);
    }
    return color * vec3f(0.9);
}

fn apply_dither(color: vec3f, uv: vec2f, enabled: f32) -> vec3f {
    if (enabled < 0.5) {
        return color;
    }

    let noise = fract(sin(dot(uv + vec2f(0.61803398875, 0.38196601125), vec2f(12.9898, 78.233))) * 43758.5453);
    let dither = (noise - 0.5) * 0.0025;
    return color + vec3f(dither);
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
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f 
{
    // 中間テクスチャの色をそのままサンプリング
    let color = textureSample(t_scene, s_scene, uv);
    
    let exposured = apply_exposure(color.xyz, u_composite.exposure);
    let saturated = apply_saturation(exposured, u_composite.saturation);
    let mapped = apply_tone_map(saturated, u_composite.tone_mapping_mode);
    let softened = apply_highlight_rolloff(mapped, u_composite.highlight_rolloff);
    let white_balanced = apply_white_point(softened, u_composite.white_point);
    let view_transformed = apply_view_transform(white_balanced, u_composite.view_transform);
    let dithered = apply_dither(view_transformed, uv, u_composite.use_dither);

    // 出力変換 (Blender 風の表示変換)
    let final_result = mix(dithered, gamma_encode(dithered), u_composite.is_use_gamma_correction);

    return vec4(final_result, 1.0);
}