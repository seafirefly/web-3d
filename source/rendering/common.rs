#![allow(dead_code)]

// Definition of GPU vertex
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub _pos: [f32; 4],
    pub _color: [f32; 3],
    pub _uv: [f32; 2],
    pub _normal: [f32; 3],
    // glTF tangent stores handedness in w. Keep it to reconstruct bitangent correctly.
    pub _tangent: [f32; 4],
}

// Imported mesh
#[derive(Clone, Default)]
pub struct Mesh {
    pub _name: std::string::String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Option<u32>,
}
