// HDR convertion to cube map textures
@group(0) @binding(0) var t_hdr: texture_2d<f32>;
@group(0) @binding(1) var s_hdr: sampler;
@group(0) @binding(2) var out_cube: texture_storage_2d_array<rgba16float, write>;

// Irradiance and Prefilter
@group(1) @binding(0) var env_cube: texture_cube<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var out_irradiance: texture_storage_2d_array<rgba16float, write>;

struct SpecularArgs { roughness: f32 };
@group(2) @binding(0) var<uniform> args : SpecularArgs;
@group(2) @binding(1) var out_prefilter : texture_storage_2d_array<rgba16float, write>;

// Mip chain generation for the HDR cube
@group(0) @binding(3) var mip_src: texture_2d_array<f32>;
@group(0) @binding(4) var mip_dst: texture_storage_2d_array<rgba16float, write>;

const PI: f32 = 3.14159265359;

// キューブマップのピクセル座標・面番号・サイズから方向ベクトルを返す関数
fn get_direction_from_cube_id(coord: vec2u, face: u32, size: vec2u) -> vec3f {
    let uv = (vec2f(coord) + 0.5) / vec2f(size) * 2.0 - 1.0;
    var dir: vec3f;
    switch (face) {
        case 0u: { dir = vec3f( 1.0, -uv.y, -uv.x); } // +X
        case 1u: { dir = vec3f(-1.0, -uv.y,  uv.x); } // -X
        case 2u: { dir = vec3f( uv.x,  1.0,  uv.y); } // +Y
        case 3u: { dir = vec3f( uv.x, -1.0, -uv.y); } // -Y
        case 4u: { dir = vec3f( uv.x, -uv.y,  1.0); } // +Z
        case 5u: { dir = vec3f(-uv.x, -uv.y, -1.0); } // -Z
        default: { dir = vec3f(0.0); }
    }
    return normalize(dir);
}


@compute @workgroup_size(16, 16, 1)
fn cs_convertion_main(@builtin(global_invocation_id) id: vec3u)
{
	let size = textureDimensions(out_cube);
    if (id.x >= size.x || id.y >= size.y) { return; }

    let face = id.z;
    let n_dir = get_direction_from_cube_id(id.xy, face, size.xy);

    // 経度: -PI ~ PI -> 0.0 ~ 1.0
    let phi = atan2(n_dir.y, n_dir.x);
    // 緯度: 0(上) ~ PI(下) -> 0.0 ~ 1.0
    let theta = acos(n_dir.z); 

    let pano_uv = vec2f(
        (phi / (2.0 * 3.14159265)) + 0.5,
        theta / 3.14159265
    );

    let color = textureSampleLevel(t_hdr, s_hdr, pano_uv, 0.0);
    textureStore(out_cube, id.xy, face, color);
}


// 2x2 box filter from the previous mip level
@compute @workgroup_size(8, 8, 1)
fn cs_downsample_main(@builtin(global_invocation_id) id: vec3u)
{
    let dst_size = textureDimensions(mip_dst).xy;
    if (any(id.xy >= dst_size)) { return; }

    let src_max = vec2i(textureDimensions(mip_src).xy) - 1;
    let base = vec2i(id.xy) * 2;
    let face = i32(id.z);
    var sum = vec4f(0.0);
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            let coord = clamp(base + vec2i(x, y), vec2i(0), src_max);
            sum += textureLoad(mip_src, coord, face, 0);
        }
    }
    textureStore(mip_dst, id.xy, id.z, sum * 0.25);
}

fn hammersley(i: u32, n: u32) -> vec2f 
{
    var bits = i;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    let radical_inverse = f32(bits) * 2.3283064365386963e-10;
    return vec2f(f32(i) / f32(n), radical_inverse);
}

fn importance_sample_ggx(xi: vec2f, n: vec3f, roughness: f32) -> vec3f 
{
    let a = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    let cos_phi   = cos(phi);
    let sin_phi   = sin(phi);

    let h = vec3f(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta);
    let up = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);

    return normalize(tangent * h.x + bitangent * h.y + n * h.z);
}

fn sample_cosine_weighted(xi: vec2f, n: vec3f) -> vec3f {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt(1.0 - xi.y);
    let sin_theta = sqrt(xi.y);

    let tangent_sample = vec3f(sin_theta * sin(phi), sin_theta * cos(phi), cos_theta);

    // 法線 N を基準とした接空間に変換
    var up = select(vec3f(0.0, 1.0, 0.0), vec3f(0.0, 0.0, 1.0), abs(n.y) > 0.999);
    let right = normalize(cross(up, n));
    up = normalize(cross(n, right));

    return tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * n;
}

// Mip level to sample for a sample with the given pdf (GPU Gems 3 filtered importance sampling)
fn sample_lod(pdf: f32, sample_count: u32) -> f32 {
    let face_size = f32(textureDimensions(env_cube, 0).x);
    let texel_solid_angle = 4.0 * PI / (6.0 * face_size * face_size);
    let sample_solid_angle = 1.0 / (f32(sample_count) * max(pdf, 1e-6));
    let max_lod = f32(textureNumLevels(env_cube) - 1u);
    return clamp(0.5 * log2(sample_solid_angle / texel_solid_angle) + 1.0, 0.0, max_lod);
}

fn random01_from_u32(seed: u32) -> f32 {
    var x = seed;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return f32(x) * 2.3283064365386963e-10;
}

@compute @workgroup_size(8, 8, 1)
fn cs_irradiance_main(@builtin(global_invocation_id) id: vec3u)
{
    let size = textureDimensions(out_irradiance).xy;
    if (any(id.xy >= size)) { return; }

    let normal = normalize(get_direction_from_cube_id(id.xy, id.z, size));
    var irradiance = vec3f(0.0);
    let sample_count = 256u;

    let sx = id.x * 73856093u;
    let sy = id.y * 19349663u;
    let sz = id.z * 83492791u;
    let seed0 = (sx ^ sy) ^ sz;
    let seed1 = seed0 ^ 2654435769u;
    let texel_offset = vec2f(random01_from_u32(seed0), random01_from_u32(seed1));

    for (var i = 0u; i < sample_count; i++)
    {
        let xi = hammersley(i, sample_count);
        let xi_rot = fract(xi + texel_offset);
        let sample_vec = sample_cosine_weighted(xi_rot, normal);
        
        let pdf = max(dot(normal, sample_vec), 0.0) / PI;
        let lod = sample_lod(pdf, sample_count);
        irradiance += textureSampleLevel(env_cube, samp, sample_vec, lod).rgb;
    }

    textureStore(out_irradiance, id.xy, id.z, vec4f(irradiance / f32(sample_count), 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn cs_prefilter_main(@builtin(global_invocation_id) id: vec3u) 
{
    let size = textureDimensions(out_prefilter).xy;
    if (any(id.xy >= size)) { return; }

    let n = normalize(get_direction_from_cube_id(id.xy, id.z, size));
    var prefiltered_color = vec3f(0.0);
    var total_weight = 0.0;
    let sample_count = 1024u;

    for (var i = 0u; i < sample_count; i++) {
        let xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(xi, n, args.roughness);
        let l = normalize(2.0 * dot(n, h) * h - n);

        let n_dot_l = max(dot(n, l), 0.0);
        if (n_dot_l > 0.0) {
            // GGX D term divided by 4 is the pdf of l when V = N
            let alpha = args.roughness * args.roughness;
            let alpha2 = alpha * alpha;
            let n_dot_h = max(dot(n, h), 0.0);
            let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
            let pdf = alpha2 / (PI * denom * denom) / 4.0;
            // Roughness 0 keeps the source resolution
            let lod = select(sample_lod(pdf, sample_count), 0.0, args.roughness < 1e-3);
            prefiltered_color += textureSampleLevel(env_cube, samp, l, lod).rgb * n_dot_l;
            total_weight += n_dot_l;
        }
    }
    textureStore(out_prefilter, id.xy, id.z, vec4f(prefiltered_color / total_weight, 1.0));
}