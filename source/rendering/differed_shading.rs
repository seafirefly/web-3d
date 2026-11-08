use crate::engine;
use crate::engine::constant;
use crate::rendering::common;
use crate::rendering::webgpu;
use crate::rendering::gpu_profiler::GpuPassGroup;
use crate::rendering::webgpu::{
    WebGPUInterface, WebGPUShaderContext, WebGPUUniqueResources, WEBGPU_CULL_MODE,
    WEBGPU_DEPTH_FORMAT,
};
use wasm_bindgen::JsCast;

// Differed shading pass -----------------------------------------------------------------------------

pub fn differed_shading_pass(
    interface: &WebGPUInterface,
    scene: &engine::scene::Scene,
    shader_map: &mut std::collections::HashMap<std::string::String, WebGPUShaderContext>,
    command_encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    // Update differed gbuffer resources
    {
        let differed_gbuffer_shader: &WebGPUShaderContext = &shader_map
            .entry("DifferedGBuffer".to_string())
            .or_insert_with(|| webgpu::create_shader_context(&interface, "DifferedGBuffer"));

        for scene_object in scene.objects.iter() {
            if scene_object.mesh_rendering_resource.is_some() {
                update_differed_gbuffer_shading_resource(
                    &interface,
                    &differed_gbuffer_shader,
                    scene,
                    &scene_object,
                );
            }
        }
    }

    // Initialize differed shading resources
    if global_resources.differed_shading_resource.is_none() {
        global_resources.differed_shading_resource = Some(create_differed_shading_context(
            &interface,
            global_resources,
        ));
    }

    // Update differed shading resources
    {
        update_differed_shading_buffer(
            &interface,
            scene,
            global_resources.shadow_resource.as_ref().unwrap(),
            &global_resources.differed_shading_resource.as_ref().unwrap(),
        );
    }

    // Rendering pass
    {
        // Render differed gbuffer writing pass
        {
            let differed_gbuffer_shader_context: &WebGPUShaderContext = &shader_map
                .get("DifferedGBuffer")
                .expect("DifferedGBuffer shader is not exist");

            let mut gbuffer_pass: wgpu::RenderPass<'_> =
                command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Differed gbuffer pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &global_resources
                                .differed_shading_resource
                                .as_ref()
                                .unwrap()
                                .gbuffer_position_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &global_resources
                                .differed_shading_resource
                                .as_ref()
                                .unwrap()
                                .gbuffer_normal_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 1.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &global_resources
                                .differed_shading_resource
                                .as_ref()
                                .unwrap()
                                .gbuffer_albedo_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &global_resources
                                .differed_shading_resource
                                .as_ref()
                                .unwrap()
                                .gbuffer_metallic_roughness_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                    ],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &interface
                            .depth_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: interface
                        .gpu_profiler
                        .timestamp_writes(GpuPassGroup::BasePass),
                    occlusion_query_set: None,
                });

            gbuffer_pass.set_pipeline(&differed_gbuffer_shader_context.render_pipeline);

            for object in scene.objects.iter() {
                if object.mesh_rendering_resource.is_none() {
                    continue;
                }

                gbuffer_pass.set_bind_group(
                    0,
                    &object
                        .mesh_rendering_resource
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .bind_group,
                    &[],
                );
                if object
                    .mesh_rendering_resource
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .bind_group_2
                    .is_some()
                {
                    gbuffer_pass.set_bind_group(
                        1,
                        &object
                            .mesh_rendering_resource
                            .as_ref()
                            .unwrap()
                            .borrow()
                            .bind_group_2,
                        &[],
                    );
                }
                gbuffer_pass.set_index_buffer(
                    object
                        .mesh_rendering_resource
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .index_buffer
                        .slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                gbuffer_pass.set_vertex_buffer(
                    0,
                    object
                        .mesh_rendering_resource
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .vertex_buffer
                        .slice(..),
                );
                gbuffer_pass.draw_indexed(
                    0..object
                        .mesh_rendering_resource
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .index_count,
                    0,
                    0..1,
                );
            }
        }

        // differed shading pass
        {
            let scene_value = scene;
            let is_debug_out = scene.parameters.differed_debug_type != 0;
            let is_use_ibl = false; // TODO: Implement IBL

            // render pass setup
            let mut differed_shading_pass: wgpu::RenderPass<'_> = command_encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Differed shading pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: scene_value.parameters.background_color[0] as f64,
                                g: scene_value.parameters.background_color[1] as f64,
                                b: scene_value.parameters.background_color[2] as f64,
                                a: scene_value.parameters.background_color[3] as f64,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: interface
                        .gpu_profiler
                        .timestamp_writes(GpuPassGroup::Lighting),
                    occlusion_query_set: None,
                });

            // set pipeline
            if !is_debug_out {
                if !is_use_ibl {
                    differed_shading_pass.set_pipeline(
                        &global_resources
                            .differed_shading_resource
                            .as_ref()
                            .unwrap()
                            .render_pipeline,
                    );
                } else {
                    differed_shading_pass.set_pipeline(
                        &global_resources
                            .differed_shading_resource
                            .as_ref()
                            .unwrap()
                            .ibl_render_pipeline
                            .as_ref()
                            .unwrap(),
                    );
                }
            } else {
                differed_shading_pass.set_pipeline(
                    &global_resources
                        .differed_shading_resource
                        .as_ref()
                        .unwrap()
                        .debug_pipeline,
                );
            }

            // set bind groups
            {
                differed_shading_pass.set_bind_group(
                    0,
                    &global_resources
                        .differed_shading_resource
                        .as_ref()
                        .unwrap()
                        .bind_groups[0],
                    &[],
                );

                differed_shading_pass.set_bind_group(
                    1,
                    &global_resources
                        .differed_shading_resource
                        .as_ref()
                        .unwrap()
                        .bind_groups[1],
                    &[],
                );

                differed_shading_pass.set_bind_group(
                    2,
                    &global_resources
                        .differed_shading_resource
                        .as_ref()
                        .unwrap()
                        .bind_groups[2],
                    &[],
                );

                if is_use_ibl {
                    differed_shading_pass.set_bind_group(
                        3,
                        &global_resources
                            .differed_shading_resource
                            .as_ref()
                            .unwrap()
                            .bind_groups[3],
                        &[],
                    );
                }
            }

            // draw call
            differed_shading_pass.draw(0..6, 0..1);
        }
    }
}

// Differed shading context ---------------------------------------------------------------------------------
pub struct WebGPUDifferedShadingResource {
    pub _shader: wgpu::ShaderModule,
    gbuffer_position_texture: wgpu::Texture,
    gbuffer_normal_texture: wgpu::Texture,
    gbuffer_albedo_texture: wgpu::Texture,
    gbuffer_metallic_roughness_texture: wgpu::Texture,
    pub bind_groups: Vec<wgpu::BindGroup>,
    pub uniform_buf: wgpu::Buffer,
    pub render_pipeline: wgpu::RenderPipeline,
    pub debug_pipeline: wgpu::RenderPipeline,
    pub ibl_render_pipeline: Option<wgpu::RenderPipeline>,
}

pub fn create_differed_gbuffer_shader_context(interface: &WebGPUInterface) -> WebGPUShaderContext {
    struct WriteGBuffersUniform {
        _model_matrix: [f32; 16],
        _view_matrix: [f32; 16],
        _projection_matrix: [f32; 16],
        _rotation_matrix: [f32; 16],
    }

    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/differed_write_gbuffers.wgsl"
                ))),
            });

    let uniform_size: u64 = std::mem::size_of::<WriteGBuffersUniform>() as u64;

    let uniform_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let texture_bind_group_layout =
        interface
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

    let pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

    let vertex_buffers: [wgpu::VertexBufferLayout<'_>; 1] = [wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<common::Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: std::mem::size_of::<[f32; 9]>() as u64,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: std::mem::size_of::<[f32; 7]>() as u64,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: std::mem::size_of::<[f32; 12]>() as u64,
                shader_location: 3,
            },
        ],
    }];

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
                    buffers: &vertex_buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(constant::FS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::all(),
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::all(),
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::all(),
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::all(),
                        }),
                    ],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(WEBGPU_CULL_MODE),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: WEBGPU_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

    return WebGPUShaderContext {
        _shader_name: "DifferedGBuffer".to_string(),
        render_pipeline,
        bind_group_layout: uniform_bind_group_layout,
        _bind_group_layout_2: Some(texture_bind_group_layout),
        _bind_group_3: None,
        uniform_size,
    };
}

fn update_differed_gbuffer_shading_resource(
    interface: &WebGPUInterface,
    shader_context: &WebGPUShaderContext,
    scene: &engine::scene::Scene,
    object: &engine::scene::SceneObject,
) {
    if object.mesh_rendering_resource.is_none() {
        log::debug!("[Update] Rendering resource is empty");
        return;
    }

    // Check if uniform buffer and bind groups are initialized
    let mut is_need_initialize: bool = false;
    {
        is_need_initialize |= object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow()
            .uniform_buffer
            .is_none();

        is_need_initialize |= object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow()
            .bind_group
            .is_none();

        is_need_initialize |= object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow()
            .bind_group_2
            .is_none();

        is_need_initialize |= object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow()
            .uniform_buffer
            .is_some()
            && object
                .mesh_rendering_resource
                .as_ref()
                .unwrap()
                .borrow()
                .uniform_buffer
                .as_ref()
                .unwrap()
                .size()
                != shader_context.uniform_size;
    }

    // Initialize uniform buffer
    if is_need_initialize {
        let uniform_buf: wgpu::Buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("[Writing GBuffer] Uniform Buffer]"),
            size: shader_context.uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow_mut()
            .uniform_buffer = Some(uniform_buf);
    }

    // Intialize bind group 1 (Uniform)
    if is_need_initialize {
        let bind_group: wgpu::BindGroup =
            interface
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &shader_context.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: object
                            .mesh_rendering_resource
                            .as_ref()
                            .unwrap()
                            .borrow()
                            .uniform_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    }],
                    label: Some("Bind group 0"),
                });
        object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow_mut()
            .bind_group = Some(bind_group);
    }

    // Initialize bind group 2 (Textures)
    if is_need_initialize {
        let bind_group_2 = interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &shader_context
                    ._bind_group_layout_2
                    .as_ref()
                    .expect("[Update] Bind group layout 2"),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .base_color_texture_view
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .base_color_texture_sampler
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .normal_texture_view
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .normal_texture_sampler
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .metallic_roughness_texture_view
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(
                            &object
                                .mesh_rendering_resource
                                .as_ref()
                                .unwrap()
                                .borrow()
                                .metallic_roughness_texture_sampler
                                .as_ref()
                                .unwrap(),
                        ),
                    },
                ],
                label: Some("texture_bind_group"),
            });

        object
            .mesh_rendering_resource
            .as_ref()
            .unwrap()
            .borrow_mut()
            .bind_group_2 = Some(bind_group_2);
    }

    // Update uniform buffer
    {
        let canvas: web_sys::Element = gloo::utils::document()
            .get_element_by_id(constant::CANVAS_ELEMENT_ID)
            .unwrap();
        let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().unwrap();
        let width: u32 = canvas.client_width() as u32;
        let height: u32 = canvas.client_height() as u32;
        let aspect_ratio: f32 = width as f32 / height as f32;

        let scene_value = scene;
        let eye: glam::Vec3 = scene_value.parameters.eye_location;
        let direction: glam::Vec3 = scene_value.parameters.eye_direction;

        let mut model_matrix = glam::Mat4::from_cols_array_2d(&object.world_transform);

        // Force Y-up to Z-up
        if scene_value.parameters.is_convert_y_to_z {
            let y_to_z_mat: glam::Mat4 = glam::Mat4::from_axis_angle(
                glam::Vec3::new(1.0, 0.0, 0.0),
                std::f32::consts::PI / 2.0,
            );
            model_matrix = y_to_z_mat * model_matrix;
        }

        // Create matrices and write buffer
        let view_matrix = glam::Mat4::look_to_rh(eye, direction, glam::Vec3::Z);
        let projection_matrix: glam::Mat4 =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect_ratio, 0.01, 100.0);

        let rotaton_matrix: glam::Mat4 =
            glam::Mat4::from_quat(model_matrix.to_scale_rotation_translation().1);

        let mut uniform_total = model_matrix.to_cols_array().to_vec();
        uniform_total.extend_from_slice(&view_matrix.to_cols_array());
        uniform_total.extend_from_slice(&projection_matrix.to_cols_array());
        uniform_total.extend_from_slice(&rotaton_matrix.to_cols_array());
        let uniform_ref: &[f32] = uniform_total.as_ref();
        interface.queue.write_buffer(
            &object
                .mesh_rendering_resource
                .as_ref()
                .unwrap()
                .borrow()
                .uniform_buffer
                .as_ref()
                .unwrap(),
            0,
            bytemuck::cast_slice(uniform_ref),
        );
    }
}

struct DifferedUniform {
    _directional_light: [f32; 4],
    _ambient_light: [f32; 4],
    _camera_position: [f32; 4],
    _light_view_projection: [f32; 16],
    _debug: [f32; 4],
}

fn create_differed_shading_context(
    interface: &WebGPUInterface,
    global_resources: &mut WebGPUUniqueResources,
) -> WebGPUDifferedShadingResource {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/differed.wgsl"
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

    // Gbuffers

    let gbuffer_position_texture: wgpu::Texture =
        interface.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("position texture"),
            size: wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

    let gbuffer_normal_texture: wgpu::Texture =
        interface.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("normal texture"),
            size: wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

    let gbuffer_albedo_texture: wgpu::Texture =
        interface.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("albedo texture"),
            size: wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

    let gbuffer_metallic_roughness_texture: wgpu::Texture =
        interface.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("metallic roughness texture"),
            size: wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

    // bindings

    let gbuffer_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

    let gbuffer_bind_group: wgpu::BindGroup =
        interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &gbuffer_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &gbuffer_position_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &gbuffer_normal_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &interface
                                .depth_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(
                            &gbuffer_albedo_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &gbuffer_metallic_roughness_texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                ],
                label: Some("Bind group 0"),
            });

    let uniform_size: u64 = std::mem::size_of::<DifferedUniform>() as u64;
    let uniform_buf: wgpu::Buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Differed uniform buffer"),
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
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
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
                    resource: uniform_buf.as_entire_binding(),
                }],
                label: Some("Bind group 1"),
            });

    let shadow_resource = global_resources
        .shadow_resource
        .as_ref()
        .expect("Shadow resource should be initialized before differed shading");

    let shadow_bind_group_layout: wgpu::BindGroupLayout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

    let shadow_bind_group: wgpu::BindGroup = interface
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &shadow_resource
                            .shadow_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_resource.shadow_sampler),
                },
            ],
            label: Some("Bind group 2"),
        });

    // pipeline

    let pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    &gbuffer_bind_group_layout,
                    &uniform_bind_group_layout,
                    &shadow_bind_group_layout,
                ],
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
                    entry_point: Some(engine::constant::FS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    targets: &[Some(interface.intermediate_texture.format().into())],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(WEBGPU_CULL_MODE),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

    let debug_pipeline: wgpu::RenderPipeline =
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
                    entry_point: Some("fs_debug_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(interface.intermediate_texture.format().into())],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(WEBGPU_CULL_MODE),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

    let mut bind_groups: Vec<wgpu::BindGroup> = Vec::new();
    bind_groups.push(gbuffer_bind_group);
    bind_groups.push(uniform_bind_group);
    bind_groups.push(shadow_bind_group);

    // ibl context
    let mut ibl_render_pipeline: Option<wgpu::RenderPipeline> = None;
    if global_resources.hdr_convertion_resource.is_some() {
        let ibl_sampler = interface.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("IBL Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let ibl_bind_group_layout =
            interface
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("IBL Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::Cube,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::Cube,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let ibl_bind_group = interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("IBL Bind Group"),
                layout: &ibl_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &global_resources
                                .hdr_convertion_resource
                                .as_ref()
                                .unwrap()
                                .irradiance_map
                                .create_view(&wgpu::TextureViewDescriptor {
                                    label: Some("Cube Sample View"),
                                    dimension: Some(wgpu::TextureViewDimension::Cube),
                                    ..Default::default()
                                }),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &global_resources
                                .hdr_convertion_resource
                                .as_ref()
                                .unwrap()
                                .prefilter_map
                                .create_view(&wgpu::TextureViewDescriptor {
                                    label: Some("Cube Sample View"),
                                    dimension: Some(wgpu::TextureViewDimension::Cube),
                                    ..Default::default()
                                }),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&ibl_sampler),
                    },
                ],
            });

        bind_groups.push(ibl_bind_group);

        let ibl_piepeline_layout =
            interface
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("IBL Pipeline Layout"),
                    bind_group_layouts: &[
                        &gbuffer_bind_group_layout,
                        &uniform_bind_group_layout,
                        &shadow_bind_group_layout,
                        &ibl_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

        ibl_render_pipeline = Some(interface.device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("IBL Render Pipeline"),
                layout: Some(&ibl_piepeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(constant::VS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_ibl_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(interface.intermediate_texture.format().into())],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(WEBGPU_CULL_MODE),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            },
        ));
    }

    return WebGPUDifferedShadingResource {
        _shader: shader,
        gbuffer_position_texture,
        gbuffer_normal_texture,
        gbuffer_albedo_texture,
        gbuffer_metallic_roughness_texture,
        bind_groups,
        uniform_buf,
        render_pipeline,
        debug_pipeline,
        ibl_render_pipeline,
    };
}

fn update_differed_shading_buffer(
    interface: &WebGPUInterface,
    scene: &engine::scene::Scene,
    shadow_resource: &crate::rendering::shadow::WebGPUShadowResource,
    resource: &WebGPUDifferedShadingResource,
) {
    let scene_value = scene;

    let eye: glam::Vec3 = scene_value.parameters.eye_location;

    let mut directional_angle =
        glam::Vec3::from_array(scene_value.parameters.light_parameters.directional_light_angle);
    if scene_value.parameters.is_convert_y_to_z {
        let y_to_z_rot = glam::Mat3::from_axis_angle(glam::Vec3::X, std::f32::consts::PI / 2.0);
        directional_angle = y_to_z_rot * directional_angle;
    }
    let directional_intensity: f32 = scene_value.parameters.light_parameters.directional_light_intensity;
    let ambient: [f32; 4] = scene_value.parameters.light_parameters.ambient_light_color;
    let camera_position: [f32; 4] = [eye.x, eye.y, eye.z, 1.0];

    let mut uniform_total: Vec<f32> = Vec::new();
    uniform_total.extend_from_slice(&directional_angle.to_array());
    uniform_total.extend_from_slice(&[directional_intensity]);
    uniform_total.extend_from_slice(&ambient);
    uniform_total.extend_from_slice(&camera_position);
    uniform_total.extend_from_slice(&shadow_resource.light_view_projection.to_cols_array());
    uniform_total.extend_from_slice(&[
        scene_value.parameters.differed_debug_type as f32,
        0.0,
        0.0,
        0.0,
    ]);

    let uniform_ref: &[f32] = uniform_total.as_ref();
    interface
        .queue
        .write_buffer(&resource.uniform_buf, 0, bytemuck::cast_slice(uniform_ref));
}
