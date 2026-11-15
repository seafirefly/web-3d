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

const PI : f32 = radians(180.0);
const GGX_D_EPSILON : f32 = 0.0000001;
const GGX_VIS_EPSILON : f32 = 0.0000001;
const SHADOW_FILTER_RADIUS : f32 = 2.0;

// pbr utility

fn distribution_GGX(normal : vec3<f32>, half : vec3<f32>, roughness : f32) -> f32
{
  let alpha    : f32 = roughness * roughness;
  let alpha2   : f32 = alpha * alpha;
    let n_dot_h  : f32 = max(dot(normal, half), 0.0);
    let n_dot_h2 : f32 = n_dot_h * n_dot_h;
	
    let nom   : f32    = alpha2;
    var denom : f32    = (n_dot_h2 * (alpha2 - 1.0) + 1.0);
    denom              = PI * denom * denom;
	
  return nom / max(denom, GGX_D_EPSILON);
}

// Smith height-correlated G2 folded into 1/(4*NdotL*NdotV) — matches Blender Principled BSDF
fn visibility_GGX(n_dot_l: f32, n_dot_v: f32, roughness: f32) -> f32
{
    let a  = roughness * roughness;
    let a2 = a * a;
    let lambda_v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let lambda_l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
  return 0.5 / max(lambda_v + lambda_l, GGX_VIS_EPSILON);
}
fn fresnel_schlick(cos_theta :f32, f0 : vec3<f32>) -> vec3<f32>
{
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

// Burley (Disney) diffuse — Blender Principled BSDF diffuse lobe
fn diffuse_burley(n_dot_l: f32, n_dot_v: f32, h_dot_v: f32, roughness: f32) -> f32
{
    let fd90 = 0.5 + 2.0 * roughness * h_dot_v * h_dot_v;
    let f_l  = 1.0 + (fd90 - 1.0) * pow(1.0 - n_dot_l, 5.0);
    let f_v  = 1.0 + (fd90 - 1.0) * pow(1.0 - n_dot_v, 5.0);
    return f_l * f_v / PI;
}

fn safe_normalize(v : vec3<f32>) -> vec3<f32>
{
  let len2 = dot(v, v);
  if (len2 <= 0.000001) {
    return vec3<f32>(0.0, 0.0, 0.0);
  }

  return v * inverseSqrt(len2);
}

struct PbrLighting
{
  direct   : vec3<f32>,
  specular : vec3<f32>,
}

fn pbr_direct_lighting(
  normal : vec3<f32>,
  view : vec3<f32>,
  light : vec3<f32>,
  albedo : vec3<f32>,
  roughness : f32,
  metallic : f32,
) -> PbrLighting
{
  let n_dot_l : f32 = max(dot(normal, light), 0.0);
  let n_dot_v : f32 = max(dot(normal, view), 0.0);

  if (n_dot_l <= 0.0 || n_dot_v <= 0.0) {
    return PbrLighting(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 0.0));
  }

  let half_vector : vec3<f32> = safe_normalize(view + light);
  let h_dot_v : f32 = max(dot(half_vector, view), 0.0);

  let base_reflectivity = vec3<f32>(0.04, 0.04, 0.04);
  let f0 = mix(base_reflectivity, albedo, vec3<f32>(metallic, metallic, metallic));
  let ndf = distribution_GGX(normal, half_vector, roughness);
  let vis = visibility_GGX(n_dot_l, n_dot_v, roughness);
  let fresnel = fresnel_schlick(h_dot_v, f0);

  let specular = ndf * vis * fresnel;

  let ks = fresnel;
  let kd = (vec3<f32>(1.0, 1.0, 1.0) - ks) * (1.0 - metallic);
  let diffuse = kd * albedo * diffuse_burley(n_dot_l, n_dot_v, h_dot_v, roughness);
  let specular_contrib = specular * n_dot_l;

  return PbrLighting((diffuse + specular) * n_dot_l, specular_contrib);
}

fn pbr_ambient_diffuse_lighting(
  normal : vec3<f32>,
  view : vec3<f32>,
  albedo : vec3<f32>,
  metallic : f32,
  ambient_irradiance : vec3<f32>,
) -> vec3<f32>
{
  let n_dot_v : f32 = max(dot(normal, view), 0.0);
  let base_reflectivity = vec3<f32>(0.04, 0.04, 0.04);
  let f0 = mix(base_reflectivity, albedo, vec3<f32>(metallic, metallic, metallic));
  let fresnel = fresnel_schlick(n_dot_v, f0);
  let kd = (vec3<f32>(1.0, 1.0, 1.0) - fresnel) * (1.0 - metallic);

  // Hemispherical weighting approximates "light coming from the environment"
  // instead of a flat additive term.
  let hemi = normal.z * 0.5 + 0.5;
  let hemi_weight = mix(0.1, 1.0, clamp(hemi, 0.0, 1.0));

  return kd * albedo * ambient_irradiance * hemi_weight / PI;
}

fn sample_directional_shadow(
  world_position: vec3<f32>,
  normal: vec3<f32>,
  light_direction: vec3<f32>,
) -> f32
{
  let shadow_clip = u_differd.light_view_projection * vec4<f32>(world_position, 1.0);
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

  let slope_bias = max(0.002 * (1.0 - max(dot(normal, light_direction), 0.0)), 0.0005);
  let shadow_depth = clamp(shadow_ndc.z - slope_bias, 0.0, 1.0);
  let shadow_dimensions = vec2<f32>(
    f32(textureDimensions(t_shadow_map).x),
    f32(textureDimensions(t_shadow_map).y)
  );
  let texel_size = 1.0 / max(shadow_dimensions, vec2<f32>(1.0, 1.0));

  var shadow_sample = 0.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 0.0, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 6.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, -2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 0.0, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 24.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, -1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 6.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 24.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 0.0, 0.0), shadow_depth) * 36.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 24.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 0.0), shadow_depth) * 6.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 0.0, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 24.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 16.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 1.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-2.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>(-1.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 0.0, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 6.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 1.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 4.0;
  shadow_sample += textureSampleCompare(t_shadow_map, s_shadow_map, shadow_uv + texel_size * vec2<f32>( 2.0 * SHADOW_FILTER_RADIUS, 2.0 * SHADOW_FILTER_RADIUS), shadow_depth) * 1.0;
  shadow_sample /= 256.0;

  return 1.0 - valid_shadow + shadow_sample * valid_shadow;
}

// render funcions

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
    let position : vec4f     = textureLoad( t_positon, vec2i(floor(coord.xy)), 0 );
    var normal   : vec3<f32> = textureLoad( t_normal, vec2i(floor(coord.xy)), 0 ).xyz;
    var depth    : f32       = textureLoad( t_depth, vec2i(floor(coord.xy)), 0 );
    var albedo   : vec4<f32> = textureLoad( t_albedo, vec2i(floor(coord.xy)), 0 );
  let metallic_roughness   : vec4<f32> = textureLoad( t_metallic, vec2i(floor(coord.xy)), 0 );

    if (depth >= 1.0) 
    {
      discard;
    }

  normal = safe_normalize(normal);

  let light_direction             : vec3<f32> = safe_normalize(-u_differd.directional_light.xyz);
  let directional_light_intensity : f32       = max(u_differd.directional_light.w, 0.0);
  let light_radiance              : f32       = directional_light_intensity;
  let view                        : vec3<f32> = safe_normalize(u_differd.camera_position.xyz - position.xyz);
  let roughness                   : f32       = clamp(metallic_roughness.g, 0.045, 1.0);
  let metallic                    : f32       = clamp(metallic_roughness.b, 0.0, 1.0);
  let shadow_visibility           : f32       = sample_directional_shadow(position.xyz, normal, light_direction);
  let lighting                    : PbrLighting = pbr_direct_lighting(
    normal,
    view,
    light_direction,
    albedo.rgb,
    roughness,
    metallic,
  );
  let direct_color                : vec3<f32> = lighting.direct;

  let ambient_diffuse : vec3<f32> = pbr_ambient_diffuse_lighting(
    normal,
    view,
    albedo.rgb,
    metallic,
    u_differd.ambient_light.rgb * max(u_differd.ambient_light.w, 0.0),
  );

  var frag_color : vec4<f32> = vec4<f32>(direct_color * light_radiance * shadow_visibility + ambient_diffuse, albedo.a);
    return frag_color;
}

@fragment
fn fs_ibl_main( @builtin(position) coord : vec4f ) -> @location(0) vec4f
{
    let position : vec4f     = textureLoad( t_positon, vec2i(floor(coord.xy)), 0 );
    var normal   : vec3<f32> = textureLoad( t_normal, vec2i(floor(coord.xy)), 0 ).xyz;
    var depth    : f32       = textureLoad( t_depth, vec2i(floor(coord.xy)), 0 );
    var albedo   : vec4<f32> = textureLoad( t_albedo, vec2i(floor(coord.xy)), 0 );
  let metallic_roughness   : vec4<f32> = textureLoad( t_metallic, vec2i(floor(coord.xy)), 0 );

    if (depth >= 1.0) 
    {
      discard;
    }

  normal = safe_normalize(normal);

  let light_direction             : vec3<f32> = safe_normalize(-u_differd.directional_light.xyz);
  let directional_light_intensity : f32       = max(u_differd.directional_light.w, 0.0);
  let light_radiance              : f32       = directional_light_intensity;
  let view                        : vec3<f32> = safe_normalize(u_differd.camera_position.xyz - position.xyz);
  let roughness                   : f32       = clamp(metallic_roughness.g, 0.045, 1.0);
  let metallic                    : f32       = clamp(metallic_roughness.b, 0.0, 1.0);
  let shadow_visibility           : f32       = sample_directional_shadow(position.xyz, normal, light_direction);
  let lighting                    : PbrLighting = pbr_direct_lighting(
    normal,
    view,
    light_direction,
    albedo.rgb,
    roughness,
    metallic,
  );
  let direct_color                : vec3<f32> = lighting.direct;

    let irradiance = textureSample(t_irradiance, s_env, normal);
  let diffuse_ibl = irradiance.rgb * albedo.rgb * (1.0 - metallic);
  let ambient_diffuse : vec3<f32> = pbr_ambient_diffuse_lighting(
    normal,
    view,
    albedo.rgb,
    metallic,
    u_differd.ambient_light.rgb * max(u_differd.ambient_light.w, 0.0),
  );

  var frag_color = vec4<f32>(direct_color * light_radiance * shadow_visibility + diffuse_ibl + ambient_diffuse, albedo.a);
    return frag_color;
}

@fragment
fn fs_debug_main( @builtin(position) coord : vec4f ) -> @location(0) vec4f
{
    let position : vec4f     = textureLoad( t_positon, vec2i(floor(coord.xy)), 0 );
    var normal   : vec3<f32> = textureLoad( t_normal, vec2i(floor(coord.xy)), 0 ).xyz;
    var depth    : f32       = textureLoad( t_depth, vec2i(floor(coord.xy)), 0 );
    let albedo   : vec4<f32> = textureLoad( t_albedo, vec2i(floor(coord.xy)), 0 );
    let metallic : vec4<f32> = textureLoad( t_metallic, vec2i(floor(coord.xy)), 0 );
    let roughness : f32      = clamp(metallic.g, 0.045, 1.0);
    let metalness : f32      = clamp(metallic.b, 0.0, 1.0);

    normal.x = (normal.x + 1.0) * 0.5;
    normal.y = (normal.y + 1.0) * 0.5;
    normal.z = (normal.z + 1.0) * 0.5;

    depth = (1.0 - depth) * 50.0;

    let shading_normal             : vec3<f32> = safe_normalize(textureLoad( t_normal, vec2i(floor(coord.xy)), 0 ).xyz);
    let light_direction            : vec3<f32> = safe_normalize(-u_differd.directional_light.xyz);
    let view                       : vec3<f32> = safe_normalize(u_differd.camera_position.xyz - position.xyz);
    let directional_light_intensity: f32       = max(u_differd.directional_light.w, 0.0);
    let light_radiance             : f32       = directional_light_intensity;
    let shadow_visibility          : f32       = sample_directional_shadow(position.xyz, shading_normal, light_direction);
    let lighting                   : PbrLighting = pbr_direct_lighting(
      shading_normal,
      view,
      light_direction,
      albedo.rgb,
      roughness,
      metalness,
    );

    // ummm
    if(u_differd.debug_params.x == 1.0)
    {
      return vec4(normal, 1.0);
    }
    else if(u_differd.debug_params.x == 2.0)
    {
      return vec4(depth, 0.0, 0.0, 1.0);
    }
    else if(u_differd.debug_params.x == 3.0)
    {
      return albedo;
    }
    else if(u_differd.debug_params.x == 4.0)
    {
      return vec4(vec3<f32>(metalness), 1.0);
    }
    else if(u_differd.debug_params.x == 5.0)
    {
      return vec4(vec3<f32>(roughness), 1.0);
    }
    else if(u_differd.debug_params.x == 6.0)
    {
      return vec4(lighting.specular * light_radiance * shadow_visibility, 1.0);
    }

    return vec4(depth, 0.0, 0.0, 1.0);
}