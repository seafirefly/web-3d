use crate::engine;
use crate::engine::constant;
use crate::rendering::common;
use crate::rendering::webgpu::{WebGPUInterface, WebGPUUniqueResources, WEBGPU_CULL_MODE};
use crate::types::Shared;

pub const SHADOW_MAP_SIZE: u32 = 4096;
pub const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_CAMERA_MAX_DISTANCE: f32 = 20.0;
const SHADOW_CAMERA_BACKWARD_MARGIN: f32 = -0.25;

pub struct WebGPUShadowResource {
    pub shadow_texture: wgpu::Texture,
    pub shadow_sampler: wgpu::Sampler,
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub light_view_projection: glam::Mat4,
}

pub fn shadow_pass(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    command_encoder: &mut wgpu::CommandEncoder,
    global_resources: &mut WebGPUUniqueResources,
) {
    if global_resources.shadow_resource.is_none() {
        global_resources.shadow_resource = Some(create_shadow_resource(interface));
    }

    let shadow_resource = global_resources.shadow_resource.as_mut().unwrap();
    shadow_resource.light_view_projection = compute_light_view_projection(scene);

    let shadow_view = shadow_resource
        .shadow_texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut shadow_render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Directional shadow pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &shadow_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    shadow_render_pass.set_pipeline(&shadow_resource.render_pipeline);

    for object in scene.borrow().objects.iter() {
        if object.mesh_rendering_resource.is_none() || object.source_mesh.is_none() {
            continue;
        }

        update_shadow_mesh_resource(interface, scene, object, shadow_resource);

        let rendering_resource = object.mesh_rendering_resource.as_ref().unwrap().borrow();
        shadow_render_pass.set_bind_group(
            0,
            rendering_resource.shadow_bind_group.as_ref().unwrap(),
            &[],
        );
        shadow_render_pass.set_index_buffer(
            rendering_resource.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        shadow_render_pass.set_vertex_buffer(0, rendering_resource.vertex_buffer.slice(..));
        shadow_render_pass.draw_indexed(0..rendering_resource.index_count, 0, 0..1);
    }
}

fn create_shadow_resource(interface: &WebGPUInterface) -> WebGPUShadowResource {
    let shader = interface
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Directional Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shader/shadow.wgsl"
            ))),
        });

    let shadow_texture = interface.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Directional Shadow Texture"),
        size: wgpu::Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHADOW_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let shadow_sampler = interface.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Directional Shadow Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        compare: Some(wgpu::CompareFunction::LessEqual),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group_layout = interface
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Directional Shadow Object Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let vertex_buffers: [wgpu::VertexBufferLayout<'_>; 1] = [wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<common::Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        }],
    }];

    let pipeline_layout = interface
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Directional Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

    let render_pipeline = interface
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Directional Shadow Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(constant::VS_ENTRY_POINT),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(WEBGPU_CULL_MODE),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: SHADOW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

    WebGPUShadowResource {
        shadow_texture,
        shadow_sampler,
        render_pipeline,
        bind_group_layout,
        light_view_projection: glam::Mat4::IDENTITY,
    }
}

fn update_shadow_mesh_resource(
    interface: &WebGPUInterface,
    scene: &Shared<engine::scene::Scene>,
    object: &engine::scene::SceneObject,
    shadow_resource: &WebGPUShadowResource,
) {
    let mesh_resource = object.mesh_rendering_resource.as_ref().unwrap();

    let needs_init = {
        let rendering_resource = mesh_resource.borrow();
        rendering_resource.shadow_uniform_buffer.is_none()
            || rendering_resource.shadow_bind_group.is_none()
    };

    if needs_init {
        let uniform_buffer = interface.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Directional Shadow Uniform Buffer"),
            size: (std::mem::size_of::<[f32; 16]>() * 2) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = interface.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Directional Shadow Bind Group"),
            layout: &shadow_resource.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let mut rendering_resource = mesh_resource.borrow_mut();
        rendering_resource.shadow_uniform_buffer = Some(uniform_buffer);
        rendering_resource.shadow_bind_group = Some(bind_group);
    }

    let scene_value = scene.borrow();
    let mut model_matrix = glam::Mat4::from_cols_array_2d(&object.world_transform);
    if scene_value.parameters.is_convert_y_to_z {
        let y_to_z_mat = glam::Mat4::from_axis_angle(glam::Vec3::X, std::f32::consts::PI / 2.0);
        model_matrix = y_to_z_mat * model_matrix;
    }

    let mut uniform_total = model_matrix.to_cols_array().to_vec();
    uniform_total.extend_from_slice(&shadow_resource.light_view_projection.to_cols_array());

    let rendering_resource = mesh_resource.borrow();
    interface.queue.write_buffer(
        rendering_resource.shadow_uniform_buffer.as_ref().unwrap(),
        0,
        bytemuck::cast_slice(uniform_total.as_slice()),
    );
}

pub fn compute_light_view_projection(scene: &Shared<engine::scene::Scene>) -> glam::Mat4 {
    let scene_value = scene.borrow();
    let mut directional = glam::Vec3::from_array(
        scene_value
            .parameters
            .light_parameters
            .directional_light_angle,
    )
    .normalize_or_zero();

    if directional.length_squared() <= 0.000001 {
        directional = glam::Vec3::new(0.5, 1.0, -1.0).normalize();
    }

    if scene_value.parameters.is_convert_y_to_z {
        let y_to_z_rot = glam::Mat3::from_axis_angle(glam::Vec3::X, std::f32::consts::PI / 2.0);
        directional = (y_to_z_rot * directional).normalize_or_zero();
    }

    let eye = scene_value.parameters.eye_location;
    let camera_forward = scene_value.parameters.eye_direction.normalize_or_zero();
    let mut world_points_visible: Vec<glam::Vec3> = Vec::new();
    let mut world_points_all: Vec<glam::Vec3> = Vec::new();

    for object in scene_value.objects.iter() {
        let source_mesh = match object.source_mesh.as_ref() {
            Some(source_mesh) => source_mesh,
            None => continue,
        };

        let mut model_matrix = glam::Mat4::from_cols_array_2d(&object.world_transform);
        if scene_value.parameters.is_convert_y_to_z {
            let y_to_z_mat = glam::Mat4::from_axis_angle(glam::Vec3::X, std::f32::consts::PI / 2.0);
            model_matrix = y_to_z_mat * model_matrix;
        }

        for vertex in source_mesh.borrow().vertices.iter() {
            let world_position = model_matrix * glam::Vec4::from_array(vertex._pos);
            let world_position = world_position.truncate();
            let to_point = world_position - eye;
            let distance = to_point.length();
            let view_alignment = if distance > 0.0001 {
                to_point / distance
            } else {
                camera_forward
            };

            world_points_all.push(world_position);
            if distance <= SHADOW_CAMERA_MAX_DISTANCE
                && camera_forward.dot(view_alignment) >= SHADOW_CAMERA_BACKWARD_MARGIN
            {
                world_points_visible.push(world_position);
            }
        }
    }

    drop(scene_value);

    let world_points = if world_points_visible.is_empty() {
        world_points_all
    } else {
        world_points_visible
    };

    if world_points.is_empty() {
        return glam::Mat4::IDENTITY;
    }

    let mut bounds_min = glam::Vec3::splat(f32::INFINITY);
    let mut bounds_max = glam::Vec3::splat(f32::NEG_INFINITY);
    for point in world_points.iter() {
        bounds_min = bounds_min.min(*point);
        bounds_max = bounds_max.max(*point);
    }

    let center = (bounds_min + bounds_max) * 0.5;
    let radius = (bounds_max - bounds_min).length().max(1.0) * 0.5;
    let light_position = center - directional * (radius * 2.0 + 1.0);
    let up = if directional.dot(glam::Vec3::Z).abs() > 0.95 {
        glam::Vec3::Y
    } else {
        glam::Vec3::Z
    };

    let light_view = glam::Mat4::look_at_rh(light_position, center, up);

    let mut light_min = glam::Vec3::splat(f32::INFINITY);
    let mut light_max = glam::Vec3::splat(f32::NEG_INFINITY);
    for point in world_points.iter() {
        let light_space_point = light_view.transform_point3(*point);
        light_min = light_min.min(light_space_point);
        light_max = light_max.max(light_space_point);
    }

    let xy_padding = 0.5;
    let z_padding = 5.0;
    light_min.x -= xy_padding;
    light_min.y -= xy_padding;
    light_max.x += xy_padding;
    light_max.y += xy_padding;

    let extent_x = (light_max.x - light_min.x).max(1.0);
    let extent_y = (light_max.y - light_min.y).max(1.0);
    let center_x = (light_min.x + light_max.x) * 0.5;
    let center_y = (light_min.y + light_max.y) * 0.5;
    let texel_size_x = extent_x / SHADOW_MAP_SIZE as f32;
    let texel_size_y = extent_y / SHADOW_MAP_SIZE as f32;
    let snapped_center_x = (center_x / texel_size_x).round() * texel_size_x;
    let snapped_center_y = (center_y / texel_size_y).round() * texel_size_y;

    light_min.x = snapped_center_x - extent_x * 0.5;
    light_max.x = snapped_center_x + extent_x * 0.5;
    light_min.y = snapped_center_y - extent_y * 0.5;
    light_max.y = snapped_center_y + extent_y * 0.5;

    let near = (-light_max.z - z_padding).max(0.1);
    let far = (-light_min.z + z_padding).max(near + 0.1);
    let light_projection = glam::Mat4::orthographic_rh(
        light_min.x,
        light_max.x,
        light_min.y,
        light_max.y,
        near,
        far,
    );

    light_projection * light_view
}