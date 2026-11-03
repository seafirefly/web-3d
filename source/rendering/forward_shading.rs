use crate::engine;
use crate::engine::constant;
use crate::rendering::common;
use crate::rendering::shadow;
use crate::rendering::webgpu::{
    WebGPUInterface, WebGPUShaderContext, WebGPUUniqueResources, WEBGPU_CULL_MODE,
    WEBGPU_DEPTH_FORMAT, WEBGPU_FRONT_FACE,
};
use crate::types::Shared;
use wasm_bindgen::JsCast;

// Forward shading pass -----------------------------------------------------------------------------

pub fn forward_shading_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    shader_map: &mut std::collections::HashMap<std::string::String, WebGPUShaderContext>,
    command_encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    global_resources: &mut WebGPUUniqueResources,
) {
    // Update phong resources
    {
        let shadow_resource = global_resources.shadow_resource.as_ref().unwrap();
        let light_view_projection = shadow_resource.light_view_projection;
        let phong_shader_context: &WebGPUShaderContext = &shader_map
            .entry("Phong".to_string())
            .or_insert(create_phong_shader_context(&interface, shadow_resource));

        for scene_object in scene.borrow().objects.iter() {
            if scene_object.mesh_rendering_resource.is_some() {
                update_phong_shading_resource(
                    &interface,
                    &phong_shader_context,
                    &scene.clone(),
                    &scene_object,
                    light_view_projection,
                );
            }
        }
    }

    // Begin forward rendering pass
    let scene_value = scene.borrow();
    let mut forward_shading_pass: wgpu::RenderPass<'_> =
        command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &interface
                    .depth_texture
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some("Depth texture view"),
                        format: Some(WEBGPU_DEPTH_FORMAT),
                        aspect: wgpu::TextureAspect::All,
                        base_array_layer: 0,
                        array_layer_count: Some(1),
                        base_mip_level: 0,
                        mip_level_count: Some(1),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                    }),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

    // Rendering forward shading
    {
        let phong_shader_context: &WebGPUShaderContext =
            &shader_map.get("Phong").expect("Phong shader is not exist");

        forward_shading_pass.set_pipeline(&phong_shader_context.render_pipeline);

        for object in scene.borrow().objects.iter() {
            if object.mesh_rendering_resource.is_none() {
                continue;
            }

            forward_shading_pass.set_bind_group(
                0,
                &object
                    .mesh_rendering_resource
                    .as_ref()
                    .expect("[draw] Rendering resource is empty")
                    .borrow()
                    .bind_group,
                &[],
            );
            forward_shading_pass.set_bind_group(
                1,
                &object
                    .mesh_rendering_resource
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .bind_group_2,
                &[],
            );
            forward_shading_pass.set_bind_group(
                2,
                phong_shader_context._bind_group_3.as_ref().unwrap(),
                &[],
            );
            forward_shading_pass.set_index_buffer(
                object
                    .mesh_rendering_resource
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .index_buffer
                    .slice(..),
                wgpu::IndexFormat::Uint32,
            );
            forward_shading_pass.set_vertex_buffer(
                0,
                object
                    .mesh_rendering_resource
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .vertex_buffer
                    .slice(..),
            );
            forward_shading_pass.draw_indexed(
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
    };
}

// Phong Shading -----------------------------------------------------------------------------

struct PhongUniform {
    _model_matrix: [f32; 16],
    _transform_matrix: [f32; 16],
    _rotation_matrix: [f32; 16],
    _directional_light: [f32; 4],
    _ambient_light: [f32; 4],
    _inverse_matrix: [f32; 16],
    _light_view_projection: [f32; 16],
    _buffer_type: [f32; 4],
}

pub fn create_phong_shader_context(
    interface: &WebGPUInterface,
    shadow_resource: &shadow::WebGPUShadowResource,
) -> WebGPUShaderContext {
    let shader: wgpu::ShaderModule =
        interface
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "../shader/phong.wgsl"
                ))),
            });

    let vertex_size: usize = std::mem::size_of::<common::Vertex>();
    let vertex_buffer_layout: [wgpu::VertexBufferLayout<'_>; 1] = [wgpu::VertexBufferLayout {
        array_stride: vertex_size as wgpu::BufferAddress,
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

    let uniform_size: u64 = std::mem::size_of::<PhongUniform>() as u64;
    let bind_group_layout: wgpu::BindGroupLayout =
        interface
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

    let bind_group_layout_2 =
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
                ],
                label: Some("bind_group_layout_2"),
            });

    let bind_group_layout_3 = interface.device.create_bind_group_layout(
        &wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout_3"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
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
        },
    );

    let bind_group_3 = interface.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Phong shadow bind group"),
        layout: &bind_group_layout_3,
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
    });

    let pipeline_layout: wgpu::PipelineLayout =
        interface
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout, &bind_group_layout_2, &bind_group_layout_3],
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
                    buffers: &vertex_buffer_layout,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(constant::FS_ENTRY_POINT),
                    compilation_options: Default::default(),
                    targets: &[Some(interface.intermediate_texture.format().into())],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: WEBGPU_FRONT_FACE,
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
        _shader_name: "Phong".to_string(),
        bind_group_layout: bind_group_layout,
        _bind_group_layout_2: Some(bind_group_layout_2),
        _bind_group_3: Some(bind_group_3),
        render_pipeline: render_pipeline,
        uniform_size: uniform_size,
    };
}

fn update_phong_shading_resource(
    interface: &WebGPUInterface,
    shader_resource: &WebGPUShaderContext,
    scene: &Shared<engine::scene::Scene>,
    object: &engine::scene::SceneObject,
    light_view_projection: glam::Mat4,
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
                != shader_resource.uniform_size;
    }

    // Initialize uniform buffer
    if is_need_initialize {
        let uniform_buf: wgpu::Buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("[Phong] Uniform Buffer"),
            size: shader_resource.uniform_size,
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

    // Initialize bind group 1(uniform buffer)
    if is_need_initialize {
        let bind_group: wgpu::BindGroup =
            interface
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &shader_resource.bind_group_layout,
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

    // Initialize bind group 2(textures and samplers)
    if is_need_initialize {
        let bind_group_2 = interface
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &shader_resource
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

        let scene_value = scene.borrow();

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

        let view_matrix = glam::Mat4::look_to_rh(eye, direction, glam::Vec3::Z);
        let projection_matrix: glam::Mat4 =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect_ratio, 0.01, 100.0);
        let transform_matrix: glam::Mat4 = projection_matrix * view_matrix * model_matrix;

        let mut directional =
            glam::Vec3::from_array(scene_value.parameters.light_parameters.directional_light_angle);
        if scene_value.parameters.is_convert_y_to_z {
            let y_to_z_rot = glam::Mat3::from_axis_angle(glam::Vec3::X, std::f32::consts::PI / 2.0);
            directional = y_to_z_rot * directional;
        }
        let ambient: [f32; 4] = scene_value.parameters.light_parameters.ambient_light_color;
        let inverse_projection: glam::Mat4 = transform_matrix.inverse();

        let rotaton_matrix: glam::Mat4 =
            glam::Mat4::from_quat(model_matrix.to_scale_rotation_translation().1);

        let mut uniform_total: Vec<f32> = transform_matrix.to_cols_array().to_vec();
        uniform_total.splice(0..0, model_matrix.to_cols_array());
        uniform_total.extend_from_slice(&rotaton_matrix.to_cols_array().to_vec());
        uniform_total.extend_from_slice(&directional.to_array());
        uniform_total.extend_from_slice(&[0.0]); // Padding!
        uniform_total.extend_from_slice(&ambient);
        uniform_total.extend_from_slice(&inverse_projection.to_cols_array().to_vec());
        uniform_total.extend_from_slice(&light_view_projection.to_cols_array());
        uniform_total.extend_from_slice(&[
            scene_value.parameters.forward_debug_type as f32,
            0.0,
            0.0,
            0.0,
        ]);

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
