use crate::engine;
use crate::engine::constant;
use crate::rendering::webgpu::{
    WebGPUInterface, WebGPUUniqueResources, WEBGPU_CULL_MODE, WEBGPU_FRONT_FACE,
};
use crate::Shared;

// composite shading pass --------------------------------------------------------------------------------------

pub fn composite_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    command_encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    if global_resources.composite_shading_resource.is_none() {
        global_resources.composite_shading_resource =
            Some(create_composite_shader_resource(&interface));
    }

    update_composite_uniform_buffer(&interface, &scene, global_resources);

    let mut composite_shading_pass: wgpu::RenderPass<'_> =
        command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blit shading pass"),
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

    composite_shading_pass.set_pipeline(
        &global_resources
            .composite_shading_resource
            .as_ref()
            .unwrap()
            .render_pipeline,
    );
    composite_shading_pass.set_bind_group(
        0,
        &global_resources
            .composite_shading_resource
            .as_ref()
            .unwrap()
            .texture_bind_group,
        &[],
    );
    composite_shading_pass.set_bind_group(
        1,
        &global_resources
            .composite_shading_resource
            .as_ref()
            .unwrap()
            .uniform_bind_group,
        &[],
    );
    composite_shading_pass.draw(0..3, 0..1);
}

// blit shader resource creation and update functions ----------------------------------------------------------------------------
pub struct WebGPUCompositeShadingResource {
    pub _shader: wgpu::ShaderModule,
    pub uniform_buffer: wgpu::Buffer,
    pub texture_bind_group: wgpu::BindGroup,
    pub uniform_bind_group: wgpu::BindGroup,
    pub render_pipeline: wgpu::RenderPipeline,
}

#[repr(C)]
pub struct CompositeUniformBuffer {
    pub _exposure: f32,
    pub _saturation: f32,
    pub _tone_mapping_mode: f32,
    pub _highlight_rolloff: f32,
    pub _white_point: f32,
    pub _view_transform: f32,
    pub _use_dither: f32,
    pub _is_use_gamma_correction: f32,
    pub _padding0: f32,
    pub _padding1: f32,
}

fn create_composite_shader_resource(interface: &WebGPUInterface) -> WebGPUCompositeShadingResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/composite.wgsl"
                ))),
            });

    let texture_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Intermediate texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 00,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Intermediate texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &interface
                                .intermediate_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&interface.device.create_sampler(
                            &wgpu::SamplerDescriptor {
                                address_mode_u: wgpu::AddressMode::ClampToEdge,
                                address_mode_v: wgpu::AddressMode::ClampToEdge,
                                address_mode_w: wgpu::AddressMode::ClampToEdge,
                                mag_filter: wgpu::FilterMode::Linear,
                                min_filter: wgpu::FilterMode::Nearest,
                                mipmap_filter: wgpu::FilterMode::Nearest,
                                ..Default::default()
                            },
                        )),
                    },
                ],
                label: Some("Line Grid Texture Bind Group"),
            });

    let uniform_buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Composite Uniform Buffer"),
        size: std::mem::size_of::<CompositeUniformBuffer>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Composite uniform buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("composite_uniform_bind_group_layout"),
            });

    let uniform_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &uniform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
                label: Some("composite_uniform_bind_group"),
            });

    let pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
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
                        blend: None,
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

    return WebGPUCompositeShadingResource {
        _shader: shader,
        uniform_buffer: uniform_buffer,
        texture_bind_group: texture_bind_group,
        uniform_bind_group: uniform_bind_group,
        render_pipeline: render_pipeline,
    };
}

fn update_composite_uniform_buffer(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    global_resources: &mut WebGPUUniqueResources,
) {
    let scene_parameters = scene.borrow();
    let tone_mapping_mode: f32 = match scene_parameters.parameters.tone_mapping_type {
        engine::scene::ToneMappingType::Off => 0.0,
        engine::scene::ToneMappingType::Aces => 1.0,
        engine::scene::ToneMappingType::Filmic => 2.0,
        engine::scene::ToneMappingType::Agx => 3.0,
    };

    let is_use_gamma_correction: f32 = if scene_parameters.parameters.is_use_gamma_correction {
        1.0
    } else {
        0.0
    };
    let exposure = scene_parameters.parameters.composite_exposure;
    let saturation = scene_parameters.parameters.composite_saturation;
    let highlight_rolloff = scene_parameters.parameters.composite_highlight_rolloff;
    let white_point = scene_parameters.parameters.composite_white_point;
    let view_transform = match scene_parameters.parameters.composite_view_transform {
        engine::scene::CompositeViewTransform::Standard => 0.0,
        engine::scene::CompositeViewTransform::Agx => 1.0,
        engine::scene::CompositeViewTransform::Filmic => 2.0,
    };
    let use_dither = if scene_parameters.parameters.composite_use_dither { 1.0 } else { 0.0 };
    drop(scene_parameters);

    interface.queue.write_buffer(
        &global_resources
            .composite_shading_resource
            .as_ref()
            .unwrap()
            .uniform_buffer,
        0,
        bytemuck::cast_slice(&[
            exposure,
            saturation,
            tone_mapping_mode,
            highlight_rolloff,
            white_point,
            view_transform,
            use_dither,
            is_use_gamma_correction,
            0.0,
        ]),
    );
}
