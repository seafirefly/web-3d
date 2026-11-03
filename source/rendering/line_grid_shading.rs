use crate::engine;
use crate::engine::constant;
use crate::rendering::webgpu::{
    WebGPUInterface, WebGPUUniqueResources, WEBGPU_CULL_MODE, WEBGPU_FRONT_FACE,
};
use crate::Shared;
use wasm_bindgen::JsCast;

// Line grid shading pass --------------------------------------------------------------------------------------

pub fn line_grid_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    command_encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    if global_resources.line_grid_shading_resource.is_none() {
        global_resources.line_grid_shading_resource =
            Some(create_line_grid_shader_resource(interface));
    }

    update_line_grid_shader_resource(
        &interface,
        &scene,
        &global_resources
            .line_grid_shading_resource
            .as_ref()
            .unwrap(),
    );

    let mut line_grid_shading_pass: wgpu::RenderPass<'_> =
        command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Line grid shading pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

    line_grid_shading_pass.set_pipeline(
        &global_resources
            .line_grid_shading_resource
            .as_ref()
            .unwrap()
            .render_pipeline,
    );
    line_grid_shading_pass.set_bind_group(
        0,
        &global_resources
            .line_grid_shading_resource
            .as_ref()
            .unwrap()
            .uniform_bind_group,
        &[],
    );
    line_grid_shading_pass.set_bind_group(
        1,
        &global_resources
            .line_grid_shading_resource
            .as_ref()
            .unwrap()
            .texture_bind_group,
        &[],
    );
    line_grid_shading_pass.draw(0..3, 0..1);
    // Assuming vertex buffer and draw call are set up elsewhere
}

// Line grid shader resource creation and update functions ----------------------------------------------------------------------------

pub struct WebGPULineGridShadingResource {
    pub _shader: wgpu::ShaderModule,
    pub uniform_bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub render_pipeline: wgpu::RenderPipeline,
}

struct LineGridUniform {
    _inv_projection_matrix: [f32; 16],
    _inv_view_matrix: [f32; 16],
    _camera_position: [f32; 4],
    _grid_spacing: f32,
    _line_thickness: f32,
    _fade_radius: f32,
    _padding: f32,
}

fn create_line_grid_shader_resource(interface: &WebGPUInterface) -> WebGPULineGridShadingResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/line.wgsl"
                ))),
            });

    let uniform_size: u64 = std::mem::size_of::<LineGridUniform>() as u64;
    let uniform_buffer: wgpu::Buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Line grid uniform buffer"),
        size: uniform_size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(uniform_size),
                },
                count: None,
            }],
        });

    let uniform_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &uniform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
                label: Some("Line Grid Uniform Bind Group"),
            });

    let texture_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Depth texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 00,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Depth,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

    let texture_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &interface
                            .depth_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                }],
                label: Some("Line Grid Texture Bind Group"),
            });

    let pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

    let render_pipeline: wgpu::RenderPipeline =
        interface
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(constant::VS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(constant::FS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: interface.swapchain_format.into(),
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: WEBGPU_FRONT_FACE,
                    cull_mode: Some(WEBGPU_CULL_MODE),
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

    return WebGPULineGridShadingResource {
        _shader: shader,
        uniform_bind_group: uniform_bind_group,
        texture_bind_group: texture_bind_group,
        uniform_buffer: uniform_buffer,
        render_pipeline: render_pipeline,
    };
}

fn update_line_grid_shader_resource(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    line_grid_shader_resource: &WebGPULineGridShadingResource,
) {
    let canvas: web_sys::Element = gloo::utils::document()
        .get_element_by_id(constant::CANVAS_ELEMENT_ID)
        .unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().unwrap();
    let width: u32 = canvas.client_width() as u32;
    let height: u32 = canvas.client_height() as u32;
    let aspect_ratio: f32 = width as f32 / height as f32;

    let scene_value = scene.borrow();

    let eye: glam::Vec3 = scene_value.parameters.eye_location;
    let direction: glam::Vec3 = scene_value.parameters.eye_direction;

    let inv_view_matrix = glam::Mat4::look_to_rh(eye, direction, glam::Vec3::Z).inverse();
    let inv_projection_matrix: glam::Mat4 =
        glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect_ratio, 0.01, 100.0)
            .inverse();

    let camera_position: [f32; 4] = [eye.x, eye.y, eye.z, 1.0];
    let grid_spacing: f32 = 100.0;
    let line_thickness: f32 = 1.0;
    let fade_radius: f32 = 1000.0;

    let mut uniform_total: Vec<f32> = Vec::new();
    uniform_total.extend_from_slice(&inv_projection_matrix.to_cols_array());
    uniform_total.extend_from_slice(&inv_view_matrix.to_cols_array());
    uniform_total.extend_from_slice(&camera_position);
    uniform_total.push(grid_spacing);
    uniform_total.push(line_thickness);
    uniform_total.push(fade_radius);
    uniform_total.push(0.0); // Padding!

    let uniform_ref: &[f32] = uniform_total.as_ref();
    interface.queue.write_buffer(
        &line_grid_shader_resource.uniform_buffer,
        0,
        bytemuck::cast_slice(uniform_ref),
    );
}
