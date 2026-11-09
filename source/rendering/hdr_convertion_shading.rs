use wgpu::util::DeviceExt;

use crate::engine::{self, constant};
use crate::rendering::webgpu::{WebGPUInterface, WebGPUUniqueResources};

// HDR conversion pass --------------------------------------------------------------------------------------

pub fn hdr_convertion_pass(
    interface: &WebGPUInterface,
    _scene: &engine::scene::Scene,
    command_encoder: &mut wgpu::CommandEncoder,
    global_resources: &mut WebGPUUniqueResources,
) {
    // Create sky shader resource and convert HDR to cube texture on the first update
    let is_first_update = global_resources.hdr_convertion_resource.is_none();
    if is_first_update {
        // The HDR file is loaded asynchronously on the first update. Conversion starts once it is ready.
        if !global_resources.sky_hdr_load_started {
            global_resources.sky_hdr_load_started = true;
            let sky_hdr_image = global_resources.sky_hdr_image.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (data, width, height) =
                    engine::load::load_hdr_file(constant::HDR_KLOPPENHEIM_02).await;
                *sky_hdr_image.borrow_mut() = Some(SkyHdrImage {
                    data,
                    width,
                    height,
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

            let workgroup_count = (512 + 15) / 16;
            convert_pass.dispatch_workgroups(workgroup_count, workgroup_count, 6);
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

            // 3.14 / 8 = 4 (ワークグループサイズ 8x8x1 の場合)
            // 6面分 (z=6) を一気に処理
            cpass.dispatch_workgroups(1024 / 8, 1024 / 8, 6);
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

            let prefilter_size = 128;
            for mip in 0..5 {
                let current_size = (prefilter_size >> mip).max(1);
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

// HDR conversion shader resource creation and update functions ----------------------------------------------------------------------------
pub struct SkyHdrImage {
    pub data: Vec<half::f16>,
    pub width: u32,
    pub height: u32,
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
                    "../shader/hdr_conversion.wgsl"
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

    let hdr_cube_texture = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Cube Target"),
        size: wgpu::Extent3d {
            width: sky_hdr_image.height / 2,
            height: sky_hdr_image.height / 2,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
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

    // 1. Irradiance Map (32x32で十分)
    let irradiance_map = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Irradiance Map"),
        size: wgpu::Extent3d {
            width: 1024,
            height: 1024,
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
            width: 128,
            height: 128,
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
    };
}
