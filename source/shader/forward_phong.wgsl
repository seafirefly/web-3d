struct VertexOutput {
    @builtin(position) position   : vec4<f32>,
	@location(0)       normal     : vec3<f32>,
    @location(1)       uv         : vec2<f32>,
    @location(2)       tangent    : vec4<f32>,
    @location(3)       world_pos  : vec3<f32>,
};

struct Uniform 
{
    model_matrix       : mat4x4<f32>,
    transform_matrix   : mat4x4<f32>,
    rotation_matrix    : mat4x4<f32>,
    directional_light  : vec4<f32>,
    ambient_light      : vec4<f32>,
    inverse_matrix     : mat4x4<f32>,
    light_view_projection : mat4x4<f32>,
    buffer_type        : vec4<f32>,
}

const SHADOW_FILTER_RADIUS : f32 = 2.0;

@group(0) @binding(0) var<uniform> inUniform          : Uniform;
@group(1) @binding(0) var          base_color_texture : texture_2d<f32>;
@group(1) @binding(1) var          base_color_sampler : sampler;
@group(1) @binding(2) var          normal_texture     : texture_2d<f32>;
@group(1) @binding(3) var          normal_sampler     : sampler;
@group(2) @binding(0) var          shadow_texture     : texture_depth_2d;
@group(2) @binding(1) var          shadow_sampler     : sampler_comparison;

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) normal  : vec3<f32>,
    @location(2) uv       : vec2<f32>,
    @location(3) tangent  : vec4<f32>,
) -> VertexOutput 
{
    let normal_world  : vec3<f32> = normalize((inUniform.rotation_matrix * vec4<f32>(normal, 0.0)).xyz);
	let tangent_world : vec3<f32> = normalize((inUniform.rotation_matrix * vec4<f32>(tangent.xyz, 0.0)).xyz);

    var output: VertexOutput;
    output.position = inUniform.transform_matrix * position;
    output.normal   = normal_world;
    output.uv       = uv;
    output.tangent  = vec4<f32>(tangent_world, tangent.w);
    output.world_pos = (inUniform.model_matrix * position).xyz;
    return output;
}

fn sample_directional_shadow(world_position: vec3<f32>, normal: vec3<f32>, light_direction: vec3<f32>) -> f32 {
    let shadow_clip = inUniform.light_view_projection * vec4<f32>(world_position, 1.0);
    let safe_w = max(abs(shadow_clip.w), 0.000001);
    let inv_w = select(-1.0 / safe_w, 1.0 / safe_w, shadow_clip.w >= 0.0);
    let shadow_ndc = shadow_clip.xyz * inv_w;
    let shadow_uv_raw = vec2<f32>(shadow_ndc.x * 0.5 + 0.5, shadow_ndc.y * -0.5 + 0.5);
    let shadow_uv = clamp(shadow_uv_raw, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let in_front = select(0.0, 1.0, shadow_clip.w > 0.0);
    let inside_uv = select(0.0, 1.0,
        shadow_uv_raw.x > 0.0 && shadow_uv_raw.x < 1.0 && shadow_uv_raw.y > 0.0 && shadow_uv_raw.y < 1.0);
    let inside_depth = select(0.0, 1.0, shadow_ndc.z > 0.0 && shadow_ndc.z < 1.0);
    let valid_shadow = in_front * inside_uv * inside_depth;

    let slope_bias = max(0.002 * (1.0 - max(dot(normal, -light_direction), 0.0)), 0.0005);
    let shadow_depth = clamp(shadow_ndc.z - slope_bias, 0.0, 1.0);
    let shadow_dimensions = vec2<f32>(
        f32(textureDimensions(shadow_texture).x),
        f32(textureDimensions(shadow_texture).y)
    );
    let texel_size = 1.0 / max(shadow_dimensions, vec2<f32>(1.0, 1.0));

    var shadow_sample = 0.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 0.0, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 6.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 0.0, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 24.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 6.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 24.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 0.0, 0.0), shadow_depth) * 36.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 24.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 6.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 0.0, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 24.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 0.0, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 6.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
    shadow_sample += textureSampleCompare(shadow_texture, shadow_sampler, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
    shadow_sample /= 256.0;

    return 1.0 - valid_shadow + shadow_sample * valid_shadow;
}

@fragment
fn fs_main(
    vertex: VertexOutput
) -> @location(0) vec4<f32> 
{
    let binormal_world : vec3<f32>   = normalize(cross(vertex.normal, vertex.tangent.xyz) * vertex.tangent.w);
	let tbn_matrix     : mat3x3<f32> = mat3x3<f32>(vertex.tangent.xyz, binormal_world, vertex.normal);

    let encoded_normal : vec3<f32> = textureSample(normal_texture, normal_sampler, vertex.uv).rgb;
    let surface_normal : vec3<f32> = normalize(encoded_normal - 0.5);

    let directional_light : vec3<f32> = normalize(inUniform.directional_light.xyz);
    let normal            : vec3<f32> = normalize(tbn_matrix * surface_normal);
    let diffuse           : f32       = max(dot(-1.0 * directional_light, normal), 0.0);
    let shadow_visibility : f32       = sample_directional_shadow(vertex.world_pos, normal, directional_light);

    let view     : vec3<f32> = normalize((inUniform.inverse_matrix * vertex.position).xyz);
    let halfway  : vec3<f32> = -normalize(directional_light.xyz + view);
    let specular : f32       = pow(max(dot(normal, halfway), 0.0), 300.0);

    let ambient_light     : vec4<f32> = inUniform.ambient_light;

    let surface_color  : vec4<f32> = textureSample(base_color_texture, base_color_sampler, vertex.uv);;
    let specular_color : vec4<f32> = vec4(1.0, 1.0, 1.0, 1.0);

        var frag_color = (diffuse * surface_color + specular * surface_color) * shadow_visibility + ambient_light;

    // ummm
        if(inUniform.buffer_type.x == 1.0)
    {
      return vec4((normal / 2.0 + 0.5), 1.0);
    }

    return frag_color;
}