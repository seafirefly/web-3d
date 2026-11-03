use crate::engine::{self, constant};
use crate::rendering::webgpu::{WebGPUInterface, WebGPUUniqueResources};
use crate::Shared;
use wasm_bindgen::JsCast;

// sky shading pass --------------------------------------------------------------------------------------

pub fn sky_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    command_encoder: &mut wgpu::CommandEncoder,
    _view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    let is_sky_enable = global_resources.hdr_convertion_resource.is_some();
    if !is_sky_enable {
        return;
    }

    // Create sky shader resource and convert HDR to cube texture on the first update
    let is_first_update = global_resources.sky_shading_resource.is_none();
    if is_first_update {
        global_resources.sky_shading_resource =
            Some(create_sky_shader_resource(&interface, global_resources));
    }

    update_sky_shader_resource(
        &interface,
        &scene,
        global_resources.sky_shading_resource.as_ref().unwrap(),
    );

    // Sky render pass
    {
        let mut sky_render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sky Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &interface
                    .intermediate_texture_2
                    .create_view(&wgpu::TextureViewDescriptor::default()),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        sky_render_pass.set_pipeline(
            &global_resources
                .sky_shading_resource
                .as_ref()
                .unwrap()
                .sky_pipeline,
        );
        sky_render_pass.set_bind_group(
            0,
            &global_resources
                .sky_shading_resource
                .as_ref()
                .unwrap()
                .sky_bind_group,
            &[],
        );

        sky_render_pass.draw(0..3, 0..1); // フルスクリーントライアングルを描画
    }
}

// sky shader resource creation and update functions ----------------------------------------------------------------------------
pub struct WebGPUSkyShadingResource {
    pub _shader: wgpu::ShaderModule,
    pub sky_uniform_buffer: wgpu::Buffer,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub sky_bind_group: wgpu::BindGroup,
}

pub struct SkyUniformBuffer {
    pub _inv_view_projection_matrix: [[f32; 4]; 4],
}

fn create_sky_shader_resource(
    interface: &WebGPUInterface,
    global_resources: &mut WebGPUUniqueResources,
) -> WebGPUSkyShadingResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/sky.wgsl"
                ))),
            });

    let hdr_sampler = interface.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    // Render sky pass

    let cube_sample_view = global_resources
        .hdr_convertion_resource
        .as_ref()
        .unwrap()
        .hdr_cube_texture
        .create_view(&wgpu::TextureViewDescriptor {
            label: Some("Cube Sample View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

    let sky_uniform_buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Sky Uniform Buffer"),
        size: std::mem::size_of::<SkyUniformBuffer>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let sky_pipeline = interface
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: interface.intermediate_texture.format(),
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None, // 深度テストはシェーダー内のdiscardで行うため不要
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

    let sky_bind_group = interface
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Bind Group"),
            layout: &sky_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sky_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &interface
                            .depth_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &interface
                            .intermediate_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&cube_sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
            ],
        });

    return WebGPUSkyShadingResource {
        _shader: shader,
        sky_uniform_buffer,
        sky_pipeline,
        sky_bind_group,
    };
}

fn update_sky_shader_resource(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    sky_shader_resource: &WebGPUSkyShadingResource,
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

    let inv_view_projection_matrix = inv_view_matrix * inv_projection_matrix;

    let mut uniform_total: Vec<f32> = Vec::new();
    uniform_total.extend_from_slice(&inv_view_projection_matrix.to_cols_array());

    let uniform_ref: &[f32] = uniform_total.as_ref();
    interface.queue.write_buffer(
        &sky_shader_resource.sky_uniform_buffer,
        0,
        bytemuck::cast_slice(uniform_ref),
    );
}
