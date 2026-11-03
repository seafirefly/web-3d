struct Uniform {
    model_matrix: mat4x4<f32>,
    light_view_projection_matrix: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> in_uniform: Uniform;

@vertex
fn vs_main(@location(0) position: vec4<f32>) -> @builtin(position) vec4<f32> {
    return in_uniform.light_view_projection_matrix * in_uniform.model_matrix * position;
}