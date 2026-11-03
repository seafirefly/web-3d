struct VertexOutput {
    @builtin(position) position   : vec4<f32>,
	@location(0)       normal     : vec3<f32>,
    @location(1)       uv         : vec2<f32>,
    @location(2)       tangent    : vec4<f32>,
    @location(3)       world_pos  : vec3<f32>,
};

struct FragmentOutput {
    @location(0) position : vec4<f32>,
    @location(1) normal   : vec4<f32>,
    @location(2) albedo   : vec4<f32>,
    @location(3) metallic : vec4<f32>,
}

struct Uniform {
    model_matrix      : mat4x4<f32>,
    view_matrix       : mat4x4<f32>,
    projection_matrix : mat4x4<f32>,
    rotation_matrix   : mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> inUniform : Uniform;
@group(1) @binding(0) var base_color_texture : texture_2d<f32>;
@group(1) @binding(1) var base_color_sampler : sampler;
@group(1) @binding(2) var normal_texture     : texture_2d<f32>;
@group(1) @binding(3) var normal_sampler     : sampler;
@group(1) @binding(4) var metallic_roughness_texture     : texture_2d<f32>;
@group(1) @binding(5) var metallic_roughness_sampler     : sampler;

@vertex
fn vs_main(
    @location(0) position : vec4<f32>,
    @location(1) normal   : vec3<f32>,
    @location(2) uv       : vec2<f32>,
    @location(3) tangent  : vec4<f32>,
) -> VertexOutput 
{
    let normal_world   = normalize((inUniform.rotation_matrix * vec4<f32>(normal, 0.0)).xyz);
	let tangent_world  = normalize((inUniform.rotation_matrix * vec4<f32>(tangent.xyz, 0.0)).xyz);

    var output : VertexOutput;

    output.position  = inUniform.projection_matrix * inUniform.view_matrix * inUniform.model_matrix * position;
    output.normal    = normal_world;
    output.uv        = uv;
    output.tangent   = vec4<f32>(tangent_world, tangent.w);
    output.world_pos = (inUniform.model_matrix * position).xyz;

    return output;
}

@fragment
fn fs_main(vertex: VertexOutput) -> FragmentOutput 
{
    let binormal_world = normalize(cross(vertex.normal, vertex.tangent.xyz) * vertex.tangent.w);
    let tbn_matrix     = mat3x3<f32>(vertex.tangent.xyz, binormal_world, vertex.normal);
    let encoded_normal = textureSample(normal_texture, normal_sampler, vertex.uv).rgb;
    let surface_normal = normalize(encoded_normal - 0.5);

	var output : FragmentOutput;

    output.position = vec4<f32>(vertex.world_pos, 1.0);
    output.normal   = vec4<f32>(normalize(tbn_matrix * surface_normal), 1.0);
    output.albedo   = textureSample(base_color_texture, base_color_sampler, vertex.uv);
    output.metallic = textureSample(metallic_roughness_texture, metallic_roughness_sampler, vertex.uv);

    return output;
}