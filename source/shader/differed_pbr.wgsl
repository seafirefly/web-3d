struct Uniform
{
    directional_light     : vec4<f32>,
    ambient_light         : vec4<f32>,
    camera_position       : vec4<f32>,
    light_view_projection : mat4x4<f32>,
    debug_params          : vec4<f32>,
}

// basic differed
@group(0) @binding(0) var           t_positon       : texture_2d<f32>;
@group(0) @binding(1) var           t_normal        : texture_2d<f32>;
@group(0) @binding(2) var           t_depth         : texture_depth_2d;
@group(0) @binding(3) var           t_albedo        : texture_2d<f32>;
@group(0) @binding(4) var           t_metallic      : texture_2d<f32>;
@group(1) @binding(0) var<uniform>  u_differd       : Uniform;
@group(2) @binding(0) var           t_shadow_map    : texture_depth_2d;
@group(2) @binding(1) var           s_shadow_map    : sampler_comparison;
// IBL
@group(3) @binding(0) var t_irradiance   : texture_cube<f32>;
@group(3) @binding(1) var t_prefilter    : texture_cube<f32>;
@group(3) @binding(2) var s_env          : sampler;

// fragment helpers

struct GBufferTexel
{
  position           : vec3<f32>,
  normal             : vec3<f32>,
  depth              : f32,
  albedo             : vec4<f32>,
  metallic_roughness : vec4<f32>,
}

struct Surface
{
  position  : vec3<f32>,
  normal    : vec3<f32>,
  albedo    : vec3<f32>,
  roughness : f32,
  metallic  : f32,
  view      : vec3<f32>,
}

fn load_gbuffer(coord : vec4<f32>) -> GBufferTexel
{
  let texel = vec2<i32>(floor(coord.xy));
  return GBufferTexel(
    textureLoad(t_positon, texel, 0).xyz,
    textureLoad(t_normal, texel, 0).xyz,
    textureLoad(t_depth, texel, 0),
    textureLoad(t_albedo, texel, 0),
    textureLoad(t_metallic, texel, 0),
  );
}

fn make_surface(g : GBufferTexel) -> Surface
{
  return Surface(
    g.position,
    safe_normalize(g.normal),
    g.albedo.rgb,
    clamp(g.metallic_roughness.g, 0.045, 1.0),
    clamp(g.metallic_roughness.b, 0.0, 1.0),
    safe_normalize(u_differd.camera_position.xyz - g.position),
  );
}

fn light_direction() -> vec3<f32>
{
  return safe_normalize(-u_differd.directional_light.xyz);
}

fn light_radiance() -> f32
{
  return max(u_differd.directional_light.w, 0.0);
}

fn ambient_irradiance() -> vec3<f32>
{
  return u_differd.ambient_light.rgb * max(u_differd.ambient_light.w, 0.0);
}

fn shadow_visibility_for(s : Surface, light : vec3<f32>) -> f32
{
  return sample_directional_shadow(t_shadow_map, s_shadow_map, u_differd.light_view_projection, s.position, s.normal, light);
}

// direct light multiplied by radiance and shadow visibility
fn direct_radiance(s : Surface) -> vec3<f32>
{
  let light = light_direction();
  let shadow_visibility = shadow_visibility_for(s, light);
  let lighting = pbr_direct_lighting(s.normal, s.view, light, s.albedo, s.roughness, s.metallic);
  return lighting.direct * light_radiance() * shadow_visibility;
}

// split-sum IBL: irradiance for diffuse, prefiltered environment for specular
fn ibl_ambient(s : Surface) -> vec3<f32>
{
  let n_dot_v : f32 = max(dot(s.normal, s.view), 0.0);
  let f0 : vec3<f32> = mix(vec3<f32>(0.04, 0.04, 0.04), s.albedo, vec3<f32>(s.metallic));
  let fresnel : vec3<f32> = fresnel_schlick(n_dot_v, f0);
  let kd : vec3<f32> = (vec3<f32>(1.0, 1.0, 1.0) - fresnel) * (1.0 - s.metallic);

  // Irradiance map stores the cosine-weighted average radiance, so it is used without an extra 1/PI
  let irradiance : vec3<f32> = textureSample(t_irradiance, s_env, s.normal).rgb;
  let diffuse_ibl : vec3<f32> = kd * s.albedo * irradiance;

  // Prefilter map has 5 mips (roughness 0..1 mapped to mip 0..4)
  let reflect_dir : vec3<f32> = reflect(-s.view, s.normal);
  let prefiltered : vec3<f32> = textureSampleLevel(t_prefilter, s_env, reflect_dir, s.roughness * 4.0).rgb;
  let env_brdf : vec2<f32> = env_brdf_approx(s.roughness, n_dot_v);
  let specular_ibl : vec3<f32> = prefiltered * (f0 * env_brdf.x + vec3<f32>(env_brdf.y));

  return diffuse_ibl + specular_ibl;
}

// render functions

@vertex
fn vs_main( @builtin(vertex_index) VertexIndex : u32 ) -> @builtin(position) vec4f 
{
  const pos = array(
    vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
    vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
  );

  return vec4f(pos[VertexIndex], 0.0, 1.0);
}

@fragment
fn fs_main( @builtin(position) coord : vec4f ) -> @location(0) vec4f
{
  let g = load_gbuffer(coord);
  if (g.depth >= 1.0)
  {
    discard;
  }

  let s = make_surface(g);
  let ambient_diffuse = pbr_ambient_diffuse_lighting(s.normal, s.view, s.albedo, s.metallic, ambient_irradiance());

  return vec4<f32>(direct_radiance(s) + ambient_diffuse, g.albedo.a);
}

@fragment
fn fs_ibl_main( @builtin(position) coord : vec4f ) -> @location(0) vec4f
{
  let g = load_gbuffer(coord);
  if (g.depth >= 1.0)
  {
    discard;
  }

  let s = make_surface(g);

  return vec4<f32>(direct_radiance(s) + ibl_ambient(s), g.albedo.a);
}

@fragment
fn fs_debug_main( @builtin(position) coord : vec4f ) -> @location(0) vec4f
{
  let g = load_gbuffer(coord);
  let s = make_surface(g);
  let mode = u32(u_differd.debug_params.x);

  switch (mode) {
    case 1u: {
      return vec4<f32>(g.normal * 0.5 + 0.5, 1.0);
    }
    case 3u: {
      return g.albedo;
    }
    case 4u: {
      return vec4<f32>(vec3<f32>(s.metallic), 1.0);
    }
    case 5u: {
      return vec4<f32>(vec3<f32>(s.roughness), 1.0);
    }
    case 6u: {
      let light = light_direction();
      let lighting = pbr_direct_lighting(s.normal, s.view, light, s.albedo, s.roughness, s.metallic);
      let shadow_visibility = shadow_visibility_for(s, light);
      return vec4<f32>(lighting.specular * light_radiance() * shadow_visibility, 1.0);
    }
    default: {
      return vec4<f32>((1.0 - g.depth) * 50.0, 0.0, 0.0, 1.0);
    }
  }
}
