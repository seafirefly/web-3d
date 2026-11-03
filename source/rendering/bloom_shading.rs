use crate::engine;
use crate::engine::constant;
use crate::rendering::webgpu::{
    WebGPUInterface, WebGPUUniqueResources, WEBGPU_CULL_MODE, WEBGPU_FRONT_FACE,
};
use crate::Shared;
use wasm_bindgen::JsCast;

// bloom shading pass --------------------------------------------------------------------------------------

pub fn bloom_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    command_encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    if global_resources.bloom_shading_resource.is_none() {
        global_resources.bloom_shading_resource = Some(create_bloom_shader_resource(&interface));
    }

    update_bloom_uniform_buffer(&interface, &scene, global_resources);

    // Extraction pass (intermidate 1 -> intermediate 2)
    {
        let mut extraction_shading_pass: wgpu::RenderPass<'_> =
            command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom extraction shading pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        extraction_shading_pass.set_pipeline(
            &global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .extraction_render_pipeline,
        );

        extraction_shading_pass.set_bind_group(
            0,
            &global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .extraction_texture_bind_group,
            &[],
        );
        extraction_shading_pass.set_bind_group(
            1,
            &global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .bloom_uniform_bind_group,
            &[],
        );
        extraction_shading_pass.draw(0..3, 0..1);
    }

    // Down-sampling pass (intermediate 2 -> sampling texture 0~5)
    {
        let sample_len = global_resources
            .bloom_shading_resource
            .as_ref()
            .unwrap()
            .down_sampling_bind_groups
            .len();

        for i in 0..(sample_len) {
            let target_sample_view: wgpu::TextureView = global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .sampling_textures
                .get(i)
                .unwrap()
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut down_sampling_pass: wgpu::RenderPass<'_> =
                command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Bloom Gaussian blur shading pass A"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_sample_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

            down_sampling_pass.set_pipeline(
                &global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .down_sampling_render_pipeline,
            );

            down_sampling_pass.set_bind_group(
                0,
                global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .down_sampling_bind_groups
                    .get(i)
                    .unwrap(),
                &[],
            );

            down_sampling_pass.draw(0..3, 0..1);
        }
    }

    // Up-sampling pass (sampling texture 5 -> sampling texture 4 -> ... -> sampling texture 0)
    {
        let sample_len = global_resources
            .bloom_shading_resource
            .as_ref()
            .unwrap()
            .up_sampling_bind_groups
            .len();

        for i in 0..(sample_len) {
            let target_sample_view: wgpu::TextureView = if i == sample_len - 1 {
                global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .bloom_texture
                    .create_view(&wgpu::TextureViewDescriptor::default())
            } else {
                global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .sampling_textures
                    .get(sample_len - 2 - i)
                    .unwrap()
                    .create_view(&wgpu::TextureViewDescriptor::default())
            };

            let mut up_sampling_pass: wgpu::RenderPass<'_> = if i != sample_len - 1 {
                command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Bloom up-sampling shading pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_sample_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
            } else {
                command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Bloom up-sampling shading pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_sample_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
            };

            up_sampling_pass.set_pipeline(
                &global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .up_sampling_render_pipeline,
            );

            up_sampling_pass.set_bind_group(
                0,
                global_resources
                    .bloom_shading_resource
                    .as_ref()
                    .unwrap()
                    .up_sampling_bind_groups
                    .get(i)
                    .unwrap(),
                &[],
            );

            up_sampling_pass.draw(0..3, 0..1);
        }
    }

    // Bloom resolve pass (intermediate 1 + bloom texture -> intermediate 2)
    {
        let mut resolve_pass: wgpu::RenderPass<'_> =
            command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom resolve shading pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &interface
                        .intermediate_texture_2
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        resolve_pass.set_pipeline(
            &global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .resolve_render_pipeline,
        );

        resolve_pass.set_bind_group(
            0,
            &global_resources
                .bloom_shading_resource
                .as_ref()
                .unwrap()
                .resolve_bind_group,
            &[],
        );

        resolve_pass.draw(0..3, 0..1);
    }
}

// bloom shader resource creation and update functions ----------------------------------------------------------------------------

pub struct WebGPUbloomShadingResource {
    pub _shader: wgpu::ShaderModule,
    pub bloom_texture: wgpu::Texture,
    pub bloom_uniform_buffer: wgpu::Buffer,
    pub bloom_uniform_bind_group: wgpu::BindGroup,
    pub extraction_texture_bind_group: wgpu::BindGroup,
    pub extraction_render_pipeline: wgpu::RenderPipeline,
    pub sampling_textures: std::vec::Vec<wgpu::Texture>,
    pub down_sampling_bind_groups: std::vec::Vec<wgpu::BindGroup>,
    pub down_sampling_render_pipeline: wgpu::RenderPipeline,
    pub up_sampling_bind_groups: std::vec::Vec<wgpu::BindGroup>,
    pub up_sampling_render_pipeline: wgpu::RenderPipeline,
    pub resolve_bind_group: wgpu::BindGroup,
    pub resolve_render_pipeline: wgpu::RenderPipeline,
}

pub struct BloomUniformBuffer {
    pub _threshold: f32,
}

fn create_bloom_shader_resource(interface: &WebGPUInterface) -> WebGPUbloomShadingResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/bloom.wgsl"
                ))),
            });

    let canvas: web_sys::Element = gloo::utils::document()
        .get_element_by_id(constant::CANVAS_ELEMENT_ID)
        .expect("Failed to get canvas element");
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into()
        .expect("Failed to dynamic cast canvas element");

    let width: u32 = canvas.client_width() as u32;
    let height: u32 = canvas.client_height() as u32;

    // General resources ----------------------------------------------------------------------------

    let bloom_texture: wgpu::Texture = interface.device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC,
        label: Some("Bloom intermediate texture"),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    });

    let bloom_sampler: wgpu::Sampler = interface.device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let uniform_buffer: wgpu::Buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Bloom uniform buffer"),
        size: std::mem::size_of::<BloomUniformBuffer>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Bloom uniform bind group layout"),
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
                label: Some("Bloom uniform bind group"),
            });

    // Extraction pass ----------------------------------------------------------------------------

    let extraction_texture_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Intermediate texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
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
                label: Some("extraction_texture_bind_group_layout"),
            });

    let extraction_texture_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &extraction_texture_bind_group_layout,
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
                        resource: wgpu::BindingResource::Sampler(&bloom_sampler),
                    },
                ],
                label: Some("Line Grid Texture Bind Group"),
            });

    let extraction_pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    &extraction_texture_bind_group_layout,
                    &uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

    let extraction_render_pipeline: wgpu::RenderPipeline =
        interface
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&extraction_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(constant::VS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_extraction_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: interface.intermediate_texture_2.format().into(),
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

    // Down sampling  pass ----------------------------------------------------------------------------

    let down_sampling_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Intermediate texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
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
                label: Some("gaussian_blur_texture_bind_group_layout"),
            });

    // Create bloom texture pyramid
    let mut sampling_textures: std::vec::Vec<wgpu::Texture> = std::vec::Vec::with_capacity(6);
    let mut sampling_bind_groups: std::vec::Vec<wgpu::BindGroup> = std::vec::Vec::with_capacity(6);
    let mut current_width: u32 = width;
    let mut current_height: u32 = height;
    for i in 0..6 {
        current_width = current_width >> i;
        current_height = current_height >> i;

        if current_width <= 16 || current_height <= 16 {
            break;
        }

        let sampling_texture = interface.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: current_width,
                height: current_height,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba16Float, // HDR必須
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: Some(&format!("Bloom sampling texture {}", i)),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            view_formats: &[],
        });

        let sampling_bind_group: wgpu::BindGroup = if i == 0 {
            interface
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &down_sampling_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &interface
                                    .intermediate_texture_2
                                    .create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&bloom_sampler),
                        },
                    ],
                    label: Some("Bloom down sampling Bind Group 0"),
                })
        } else {
            interface
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &down_sampling_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &sampling_textures
                                    .last()
                                    .unwrap()
                                    .create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&bloom_sampler),
                        },
                    ],
                    label: Some(&format!("Bloom down sampling Bind Group {}", i)),
                })
        };

        sampling_textures.push(sampling_texture);
        sampling_bind_groups.push(sampling_bind_group);
    }

    let down_sampling_pipeline_layout: wgpu::PipelineLayout = interface
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&down_sampling_bind_group_layout],
            push_constant_ranges: &[],
        });

    let down_sampling_render_pipeline: wgpu::RenderPipeline = interface
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&down_sampling_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(constant::VS_ENTRY_POINT),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_down_sampling_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: interface.intermediate_texture_2.format().into(),
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

    // Up sampling pass

    let up_sampling_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // Sampling texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Sampling texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("up_sampling_bind_group_layout"),
            });

    let sampling_length = sampling_bind_groups.len();
    let mut up_sampling_bind_groups: std::vec::Vec<wgpu::BindGroup> =
        std::vec::Vec::with_capacity(sampling_length);
    for i in 0..sampling_length {
        let up_sampling_bind_group: wgpu::BindGroup =
            interface
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &up_sampling_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &sampling_textures
                                    .get(sampling_length - 1 - i)
                                    .unwrap()
                                    .create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&bloom_sampler),
                        },
                    ],
                    label: Some(&format!("Bloom up sampling Bind Group {}", i)),
                });
        up_sampling_bind_groups.push(up_sampling_bind_group);
    }

    let up_sampling_pipeline_layout: wgpu::PipelineLayout = interface
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&up_sampling_bind_group_layout],
            push_constant_ranges: &[],
        });

    let up_sampling_render_pipeline: wgpu::RenderPipeline = interface
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&up_sampling_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(constant::VS_ENTRY_POINT),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_up_sampling_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: interface.intermediate_texture_2.format().into(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            // 出力 = (ソース * 1.0) + (デスティネーション * 1.0)
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
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

    // Resolve shading pass

    let resolve_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // Base texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Base texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Bloom texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
            label: Some("resolve_bind_group_layout"),
        });

    let resolve_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &resolve_bind_group_layout,
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
                        resource: wgpu::BindingResource::Sampler(&bloom_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &bloom_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                ],
                label: Some("Bloom resolve bind group"),
            });

    let resolve_pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&resolve_bind_group_layout],
                push_constant_ranges: &[],
            });

    let resolve_render_pipeline: wgpu::RenderPipeline =
        interface
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&resolve_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(constant::VS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_resolve_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: interface.intermediate_texture_2.format().into(),
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

    return WebGPUbloomShadingResource {
        _shader: shader,
        bloom_texture,
        bloom_uniform_buffer: uniform_buffer,
        bloom_uniform_bind_group: uniform_bind_group,
        extraction_texture_bind_group,
        extraction_render_pipeline,
        sampling_textures,
        down_sampling_bind_groups: sampling_bind_groups,
        down_sampling_render_pipeline,
        up_sampling_bind_groups,
        up_sampling_render_pipeline,
        resolve_bind_group,
        resolve_render_pipeline,
    };
}

fn update_bloom_uniform_buffer(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    global_resources: &mut WebGPUUniqueResources,
) {
    let bloom_threshold: f32 = scene.borrow().parameters.bloom_threshold;

    interface.queue.write_buffer(
        &global_resources
            .bloom_shading_resource
            .as_ref()
            .unwrap()
            .bloom_uniform_buffer,
        0,
        bytemuck::cast_slice(&[bloom_threshold]),
    );
}
