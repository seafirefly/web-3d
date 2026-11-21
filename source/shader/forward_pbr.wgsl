struct VertexOutput {
    @builtin(position) position   : vec4<f32>,
	@location(0)       normal     : vec3<f32>,
    @location(1) uv               : vec2<f32>,
    @location(2) tangent          : vec4<f32>,
    @location(3) world_pos        : vec3<f32>,
};

struct Uniform 
{
    model_matrix          : mat4x4<f32>,
    transform_matrix      : mat4x4<f32>,
    rotation_matrix       : mat4x4<f32>,
    directional_light     : vec4<f32>,
    ambient_light         : vec4<f32>,
    // unused; kept so the buffer layout matches forward_shading.rs
    inverse_matrix        : mat4x4<f32>,
    light_view_projection : mat4x4<f32>,
    buffer_type           : vec4<f32>,
    camera_position       : vec4<f32>,
}

@group(0) @binding(0) var<uniform> inUniform          : Uniform;
@group(1) @binding(0) var          base_color_texture : texture_2d<f32>;
@group(1) @binding(1) var          base_color_sampler : sampler;
@group(1) @binding(2) var          normal_texture     : texture_2d<f32>;
@group(1) @binding(3) var          normal_sampler     : sampler;
@group(1) @binding(4) var          metallic_roughness_texture : texture_2d<f32>;
@group(1) @binding(5) var          metallic_roughness_sampler : sampler;
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

@fragment
fn fs_main(
    vertex: VertexOutput
) -> @location(0) vec4<f32> 
{
    let binormal_world : vec3<f32> = normalize(cross(vertex.normal, vertex.tangent.xyz) * vertex.tangent.w);
    let tbn_matrix     : mat3x3<f32> = mat3x3<f32>(vertex.tangent.xyz, binormal_world, vertex.normal);

    let encoded_normal : vec3<f32> = textureSample(normal_texture, normal_sampler, vertex.uv).rgb;
    let surface_normal : vec3<f32> = normalize(encoded_normal - 0.5);
    let normal         : vec3<f32> = normalize(tbn_matrix * surface_normal);

    if (inUniform.buffer_type.x == 1.0)
    {
      return vec4<f32>(normal / 2.0 + 0.5, 1.0);
    }

    // direction towards the light
    let light     : vec3<f32> = safe_normalize(-inUniform.directional_light.xyz);
    let view      : vec3<f32> = safe_normalize(inUniform.camera_position.xyz - vertex.world_pos);
    let radiance  : f32       = max(inUniform.directional_light.w, 0.0);
    let ambient_irradiance : vec3<f32> = inUniform.ambient_light.rgb * max(inUniform.ambient_light.w, 0.0);

    let surface_color : vec4<f32> = textureSample(base_color_texture, base_color_sampler, vertex.uv);
    let albedo        : vec3<f32> = surface_color.rgb;
    let mr            : vec4<f32> = textureSample(metallic_roughness_texture, metallic_roughness_sampler, vertex.uv);
    let roughness     : f32       = clamp(mr.g, 0.045, 1.0);
    let metallic      : f32       = clamp(mr.b, 0.0, 1.0);

    let shadow_visibility : f32 = sample_directional_shadow(
        shadow_texture, shadow_sampler, inUniform.light_view_projection,
        vertex.world_pos, normal, light);

    let lighting       = pbr_direct_lighting(normal, view, light, albedo, roughness, metallic);
    let direct_color   = lighting.direct * radiance * shadow_visibility;
    let ambient_color  = pbr_ambient_diffuse_lighting(normal, view, albedo, metallic, ambient_irradiance);

    return vec4<f32>(direct_color + ambient_color, 1.0);
}
