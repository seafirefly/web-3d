use wgpu::util::DeviceExt;

use crate::engine::{self, scene::SkyBoxType};
use crate::rendering::webgpu::{WebGPUInterface, WebGPUUniqueResources};
use crate::types::Shared;

// Sky box selection ------------------------------------------------------------------------------------------

// Applies a sky box change requested from the GUI. The HDR is reloaded and converted by the next pass.
pub fn update_sky_box(
    scene: &Shared<engine::scene::Scene>,
    global_resources: &mut WebGPUUniqueResources,
) {
    let Some(sky_box_type) = scene.borrow_mut().parameters.requested_sky_box_type.take() else {
        return;
    };
    if sky_box_type == scene.borrow().parameters.sky_box_type {
        return;
    }
    scene.borrow_mut().parameters.sky_box_type = sky_box_type;

    global_resources.hdr_convertion_resource = None;
    global_resources.differed_shading_resource = None;
    global_resources.sky_shading_resource = None;
    *global_resources.sky_hdr_image.borrow_mut() = None;
    global_resources.sky_hdr_load_started = false;
}

// HDR conversion pass --------------------------------------------------------------------------------------

pub fn hdr_convertion_pass(
    interface: &WebGPUInterface,
    scene: &engine::scene::Scene,
    command_encoder: &mut wgpu::CommandEncoder,
    global_resources: &mut WebGPUUniqueResources,
) {
    // Create sky shader resource and convert HDR to cube texture on the first update
    let is_first_update = global_resources.hdr_convertion_resource.is_none();
    if is_first_update {
        let sky_box_type = scene.parameters.sky_box_type;

        // Discard an image loaded for a sky box that is no longer selected (its load may have finished late)
        let is_stale = global_resources
            .sky_hdr_image
            .borrow()
            .as_ref()
            .is_some_and(|image| image.sky_box_type != sky_box_type);
        if is_stale {
            *global_resources.sky_hdr_image.borrow_mut() = None;
            global_resources.sky_hdr_load_started = false;
        }

        // The HDR file is loaded asynchronously on the first update. Conversion starts once it is ready.
        if !global_resources.sky_hdr_load_started {
            global_resources.sky_hdr_load_started = true;
            let sky_hdr_image = global_resources.sky_hdr_image.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (data, width, height) =
                    engine::asset_loader::load_hdr_file(sky_box_type.path()).await;
                *sky_hdr_image.borrow_mut() = Some(SkyHdrImage {
                    data,
                    width,
                    height,
                    sky_box_type,
                });
            });
            return;
        }

        let sky_hdr_image_cell = global_resources.sky_hdr_image.clone();
        let sky_hdr_image_ref = sky_hdr_image_cell.borrow();
        let Some(sky_hdr_image) = sky_hdr_image_ref.as_ref() else {
            return;
        };
        global_resources.hdr_convertion_resource = Some(create_hdr_conversion_shader_resource(
            &interface,
            sky_hdr_image,
        ));
        // The differed shading bind groups reference the IBL textures, so they must be rebuilt.
        global_resources.differed_shading_resource = None;

        let cube_size = global_resources
            .hdr_convertion_resource
            .as_ref()
            .unwrap()
            .hdr_cube_texture
            .width();

        // Convert hdr to cube pass
        {
            let mut convert_pass =
                command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("HDR Conversion Pass"),
                    timestamp_writes: None,
                });

            convert_pass.set_pipeline(
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .hdr_convert_pipeline,
            );
            convert_pass.set_bind_group(
                0,
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .hdr_convert_bind_group,
                &[],
            );

            let workgroup_count = (cube_size + 15) / 16;
            convert_pass.dispatch_workgroups(workgroup_count, workgroup_count, 6);
        }

        // Build the cube mip chain so that the convolutions can sample filtered LODs
        {
            let resource = global_resources.hdr_convertion_resource.as_ref().unwrap();
            for (i, bind_group) in resource.downsample_bind_groups.iter().enumerate() {
                let mip_size = (cube_size >> (i + 1)).max(1);
                let dispatch_size = (mip_size + 7) / 8;
                let mut dpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Cube Mip Downsample Pass"),
                    timestamp_writes: None,
                });
                dpass.set_pipeline(&resource.downsample_pipeline);
                dpass.set_bind_group(0, bind_group, &[]);
                dpass.dispatch_workgroups(dispatch_size, dispatch_size, 6);
            }
        }

        // Create Irradiance map
        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Irradiance Compute Pass"),
                ..Default::default()
            });
            cpass.set_pipeline(
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .irradiance_pipeline,
            );
            cpass.set_bind_group(
                1,
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .irradiance_bind_group,
                &[],
            );

            // Workgroup size is 8x8x1 and the irradiance map is IRRADIANCE_MAP_SIZE per face; z=6 covers all faces
            cpass.dispatch_workgroups(
                IRRADIANCE_MAP_SIZE / 8,
                IRRADIANCE_MAP_SIZE / 8,
                6,
            );
        }

        // Create Prefilter map
        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Prefilter Compute Pass"),
                ..Default::default()
            });

            cpass.set_pipeline(
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .prefilter_pipeline,
            );
            cpass.set_bind_group(
                1,
                &global_resources
                    .hdr_convertion_resource
                    .as_ref()
                    .unwrap()
                    .prefilter_cubemap_bind_group,
                &[],
            );

            for mip in 0..5 {
                let current_size = (PREFILTER_MAP_SIZE >> mip).max(1);
                let dispatch_size = (current_size + 7) / 8;

                cpass.set_bind_group(
                    2,
                    &global_resources
                        .hdr_convertion_resource
                        .as_ref()
                        .unwrap()
                        .prefilter_bind_groups[mip],
                    &[],
                );
                cpass.dispatch_workgroups(dispatch_size, dispatch_size, 6);
            }
        }
    }
}

// IBL debug output ------------------------------------------------------------------------------------------

// Draws the irradiance map or a prefilter map mip as a cube cross over the given view (GUI "IBL out").
pub fn ibl_debug_pass(
    interface: &WebGPUInterface,
    scene: &engine::scene::Scene,
    command_encoder: &mut wgpu::CommandEncoder,
    global_resources: &WebGPUUniqueResources,
    view: &wgpu::TextureView,
) {
    let debug_type = scene.parameters.ibl_debug_type;
    let Some(resource) = global_resources.hdr_convertion_resource.as_ref() else {
        return;
    };
    if debug_type == 0 {
        return;
    }

    let use_prefilter = if debug_type == 1 { 0.0 } else { 1.0 };
    let lod = debug_type.saturating_sub(2) as f32;
    interface.queue.write_buffer(
        &resource.ibl_debug_uniform_buffer,
        0,
        bytemuck::cast_slice(&[use_prefilter, lod, 0.0, 0.0]),
    );

    let mut pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("IBL Debug Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
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
    pass.set_pipeline(&resource.ibl_debug_pipeline);
    pass.set_bind_group(0, &resource.ibl_debug_bind_group, &[]);
    pass.draw(0..3, 0..1);
}

// HDR conversion shader resource creation and update functions ----------------------------------------------------------------------------
const IRRADIANCE_MAP_SIZE: u32 = 32;
const PREFILTER_MAP_SIZE: u32 = 256;

pub struct SkyHdrImage {
    pub data: Vec<half::f16>,
    pub width: u32,
    pub height: u32,
    pub sky_box_type: SkyBoxType,
}

pub struct WebGPUHDREnvironmentResource {
    pub _shader: wgpu::ShaderModule,
    pub _sky_hdr_texture: wgpu::Texture,
    pub hdr_cube_texture: wgpu::Texture,
    pub irradiance_map: wgpu::Texture,
    pub prefilter_map: wgpu::Texture,
    pub hdr_convert_bind_group: wgpu::BindGroup,
    pub hdr_convert_pipeline: wgpu::ComputePipeline,
    pub irradiance_bind_group: wgpu::BindGroup,
    pub irradiance_pipeline: wgpu::ComputePipeline,
    pub prefilter_pipeline: wgpu::ComputePipeline,
    pub prefilter_cubemap_bind_group: wgpu::BindGroup,
    pub prefilter_bind_groups: Vec<wgpu::BindGroup>,
    pub downsample_pipeline: wgpu::ComputePipeline,
    // Index i writes cube mip i + 1 from mip i
    pub downsample_bind_groups: Vec<wgpu::BindGroup>,
    pub ibl_debug_pipeline: wgpu::RenderPipeline,
    pub ibl_debug_bind_group: wgpu::BindGroup,
    pub ibl_debug_uniform_buffer: wgpu::Buffer,
}

fn create_hdr_conversion_shader_resource(
    interface: &WebGPUInterface,
    sky_hdr_image: &SkyHdrImage,
) -> WebGPUHDREnvironmentResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../../shader/skybox_hdr_convertion.wgsl"
                ))),
            });

    let sky_hdr_texture = interface.device.create_texture_with_data(
        &interface.queue,
        &wgpu::TextureDescriptor {
            label: Some("Sky HDR Texture"),
            size: wgpu::Extent3d {
                width: sky_hdr_image.width,
                height: sky_hdr_image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::default(),
        bytemuck::cast_slice(&sky_hdr_image.data),
    );
    let hdr_texture_view = sky_hdr_texture.create_view(&Default::default());

    let cube_size = sky_hdr_image.height / 2;
    let cube_mip_count = 32 - cube_size.leading_zeros();
    let hdr_cube_texture = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Cube Target"),
        size: wgpu::Extent3d {
            width: cube_size,
            height: cube_size,
            depth_or_array_layers: 6,
        },
        mip_level_count: cube_mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float, // HDR精度を維持
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let hdr_sampler = interface.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    // 1. Irradiance Map (low resolution is sufficient for diffuse lighting)
    let irradiance_map = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Irradiance Map"),
        size: wgpu::Extent3d {
            width: IRRADIANCE_MAP_SIZE,
            height: IRRADIANCE_MAP_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // 2. Pre-filter Map (Mipmapが必要)
    let prefilter_map = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Pre-filter Map"),
        size: wgpu::Extent3d {
            width: PREFILTER_MAP_SIZE,
            height: PREFILTER_MAP_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: 5,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // 1. Hdr conversion to cube map pass

    let cube_storage_view = hdr_cube_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        base_mip_level: 0,
        mip_level_count: Some(1),
        ..Default::default()
    });

    let hdr_convert_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HDR Convert Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                        },
                        count: None,
                    },
                ],
            });

    let hdr_convert_bind_group = interface
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &hdr_convert_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hdr_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&cube_storage_view),
                },
            ],
        });

    let hdr_convert_pipeline_layout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("HDR Convert Pipeline Layout"),
                bind_group_layouts: &[&hdr_convert_bind_group_layout],
                push_constant_ranges: &[],
            });

    let hdr_convert_pipeline =
        interface
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&hdr_convert_pipeline_layout),
                module: &shader,
                entry_point: Some("cs_convertion_main"),
                compilation_options: Default::default(),
                cache: None,
            });

    // 2. Irradiance Map Compute pass
    let cube_sample_view = hdr_cube_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Cube Sample View"),
        dimension: Some(wgpu::TextureViewDimension::Cube), // ここをCubeにする！
        ..Default::default()
    });

    // Mip chain generation for the cube (mip 0 is written by the convert pass)
    let downsample_pipeline =
        interface
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Cube Downsample Pipeline"),
                layout: None,
                module: &shader,
                entry_point: Some("cs_downsample_main"),
                compilation_options: Default::default(),
                cache: None,
            });
    let mut downsample_bind_groups = Vec::new();
    for mip in 1..cube_mip_count {
        let src_view = hdr_cube_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_mip_level: mip - 1,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let dst_view = hdr_cube_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_mip_level: mip,
            mip_level_count: Some(1),
            ..Default::default()
        });
        downsample_bind_groups.push(interface.device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: Some("Cube Downsample Bind Group"),
                layout: &downsample_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&dst_view),
                    },
                ],
            },
        ));
    }

    let irradiance_pipeline =
        interface
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Irradiance Compute Pipeline"),
                layout: None,
                module: &shader,
                entry_point: Some("cs_irradiance_main"),
                compilation_options: Default::default(),
                cache: None,
            });

    let irradiance_bind_group = interface
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Irradiance Bind Group"),
            layout: &irradiance_pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cube_sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&irradiance_map.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2Array),
                            ..Default::default()
                        },
                    )),
                },
            ],
        });

    // 3. Pre-filter Map Compute pass

    let prefilter_pipeline =
        interface
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Pre-filter Compute Pipeline"),
                layout: None,
                module: &shader,
                entry_point: Some("cs_prefilter_main"),
                compilation_options: Default::default(),
                cache: None,
            });

    let prefilter_cubemap_bind_group =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Prefilter Bind Group"),
                layout: &prefilter_pipeline.get_bind_group_layout(1),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&cube_sample_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                    },
                ],
            });

    let mut prefilter_bind_groups = Vec::new();
    let mip_levels = 5;
    for mip in 0..mip_levels {
        let roughness = mip as f32 / (mip_levels - 1) as f32;

        // Roughnessを渡す Uniform Buffer
        let buffer = interface
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Roughness Buffer Mip {}", mip)),
                contents: bytemuck::cast_slice(&[roughness]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // 特定のMipレベルだけを指す D2Array ビュー
        let mip_view = prefilter_map.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba16Float),
            dimension: Some(wgpu::TextureViewDimension::D2Array), // 重要: Storage用なのでD2Array
            base_mip_level: mip,
            mip_level_count: Some(1),
            ..Default::default()
        });

        let bind_group = interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &prefilter_pipeline.get_bind_group_layout(2),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&mip_view),
                    },
                ],
                label: None,
            });
        prefilter_bind_groups.push(bind_group);
    }

    let ibl_debug_shader = interface
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("IBL Debug Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../../shader/ibl_debug.wgsl"
            ))),
        });
    let ibl_debug_pipeline =
        interface
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("IBL Debug Pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &ibl_debug_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &ibl_debug_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(interface.intermediate_texture.format().into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

    let ibl_debug_uniform_buffer =
        interface
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("IBL Debug Uniform Buffer"),
                contents: bytemuck::cast_slice(&[0.0f32; 4]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

    let ibl_debug_bind_group = interface
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("IBL Debug Bind Group"),
            layout: &ibl_debug_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&irradiance_map.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&prefilter_map.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ibl_debug_uniform_buffer.as_entire_binding(),
                },
            ],
        });

    return WebGPUHDREnvironmentResource {
        _shader: shader,
        _sky_hdr_texture: sky_hdr_texture,
        hdr_cube_texture,
        irradiance_map,
        prefilter_map,
        hdr_convert_bind_group,
        hdr_convert_pipeline,
        irradiance_bind_group,
        irradiance_pipeline,
        prefilter_pipeline,
        prefilter_cubemap_bind_group,
        prefilter_bind_groups,
        downsample_pipeline,
        downsample_bind_groups,
        ibl_debug_pipeline,
        ibl_debug_bind_group,
        ibl_debug_uniform_buffer,
    };
}
