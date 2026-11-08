use crate::engine::{self, constant};
use crate::rendering::common;
use crate::rendering::{
    bloom_shading, composite_shading, differed_shading, forward_shading, gpu_profiler,
    hdr_convertion_shading, line_grid_shading, shadow, sky_shading,
};
use wasm_bindgen::JsCast;
use wgpu::util::DeviceExt;

pub const WEBGPU_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
pub const WEBGPU_FRONT_FACE: wgpu::FrontFace = wgpu::FrontFace::Ccw;
pub const WEBGPU_CULL_MODE: wgpu::Face = wgpu::Face::Back;

// --------------------------------------------------------------------------------------------

/// WebGPU interface contains webgpu contexts such as device, queue, and surface. It is shared between objects.
pub struct WebGPUInterface<'a> {
    pub surface: wgpu::Surface<'a>,
    pub _adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub swapchain_format: wgpu::TextureFormat,
    pub depth_texture: wgpu::Texture,
    pub intermediate_texture: wgpu::Texture,
    pub intermediate_texture_2: wgpu::Texture,
    pub sky_hdr_texture: wgpu::Texture,
    pub gpu_profiler: gpu_profiler::GpuProfiler,
}

/// shader resource contains shader module and pipeline layout. It is shared between objects.
pub struct WebGPUShaderContext {
    pub _shader_name: std::string::String,
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub _bind_group_layout_2: Option<wgpu::BindGroupLayout>,
    pub _bind_group_3: Option<wgpu::BindGroup>,
    pub uniform_size: u64,
}

/// rendering resource contains GPU buffers and bind groups. It is created for each object.
pub struct WebGPUMeshRenderingResource {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub uniform_buffer: Option<wgpu::Buffer>,
    pub bind_group: Option<wgpu::BindGroup>,
    pub bind_group_2: Option<wgpu::BindGroup>,
    pub shadow_uniform_buffer: Option<wgpu::Buffer>,
    pub shadow_bind_group: Option<wgpu::BindGroup>,
    pub _base_color_texture: Option<wgpu::Texture>,
    pub base_color_texture_view: Option<wgpu::TextureView>,
    pub base_color_texture_sampler: Option<wgpu::Sampler>,
    pub _normal_texture: Option<wgpu::Texture>,
    pub normal_texture_view: Option<wgpu::TextureView>,
    pub normal_texture_sampler: Option<wgpu::Sampler>,
    pub _metallic_roughness_texture: Option<wgpu::Texture>,
    pub metallic_roughness_texture_view: Option<wgpu::TextureView>,
    pub metallic_roughness_texture_sampler: Option<wgpu::Sampler>,
}

// Unique resources that are shared in the whole rendering process. It is created once and shared between objects.
pub struct WebGPUUniqueResources {
    pub differed_shading_resource: Option<differed_shading::WebGPUDifferedShadingResource>,
    pub line_grid_shading_resource: Option<line_grid_shading::WebGPULineGridShadingResource>,
    pub bloom_shading_resource: Option<bloom_shading::WebGPUbloomShadingResource>,
    pub composite_shading_resource: Option<composite_shading::WebGPUCompositeShadingResource>,
    pub hdr_convertion_resource: Option<hdr_convertion_shading::WebGPUHDREnvironmentResource>,
    pub sky_shading_resource: Option<sky_shading::WebGPUSkyShadingResource>,
    pub shadow_resource: Option<shadow::WebGPUShadowResource>,
}

// Webgpu Contexts -----------------------------------------------------------------------------

/// Initialize WebGPU interface. It creates surface, device, queue, and depth texture, and returns them as WebGPUInterface struct.
pub async fn init_interface<'a>() -> WebGPUInterface<'a> {
    let canvas: web_sys::Element = gloo::utils::document()
        .get_element_by_id(constant::CANVAS_ELEMENT_ID)
        .expect("Failed to get canvas element");
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into()
        .expect("Failed to dynamic cast canvas element");

    let width: u32 = canvas.client_width() as u32;
    let height: u32 = canvas.client_height() as u32;

    // Initialize webgpu

    let instance: wgpu::Instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let surface_target = wgpu::SurfaceTarget::Canvas(canvas);

    let surface: wgpu::Surface = instance
        .create_surface(surface_target)
        .expect("Failed to create surface from canvas");

    let adapter: wgpu::Adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to request adapter");

    let timestamp_supported: bool = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let required_features: wgpu::Features = if timestamp_supported {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await
        .expect("Failed to request device");

    let swapchain_capabilities: wgpu::SurfaceCapabilities = surface.get_capabilities(&adapter);
    let swapchain_format: wgpu::TextureFormat = swapchain_capabilities.formats[0];

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        format: swapchain_format,
        width: width,
        height: height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &config);

    let depth_texture: wgpu::Texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size: wgpu::Extent3d {
            width: width,
            height: height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WEBGPU_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let intermediate_texture: wgpu::Texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Intermediate texture"),
        size: wgpu::Extent3d {
            width: width,
            height: height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let intermediate_texture_2: wgpu::Texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Intermediate texture"),
        size: wgpu::Extent3d {
            width: width,
            height: height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let (sky_hdr_data, sky_hdr_width, sky_hdr_height) =
        engine::load::load_hdr_file(constant::HDR_KLOPPENHEIM_02).await;
    let sky_hdr_texture = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: Some("Sky HDR Texture"),
            size: wgpu::Extent3d {
                width: sky_hdr_width,
                height: sky_hdr_height,
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
        bytemuck::cast_slice(&sky_hdr_data),
    );

    let gpu_profiler = gpu_profiler::GpuProfiler::new(&device, &queue, timestamp_supported);

    // Return webgpu resource

    let interface: WebGPUInterface<'_> = WebGPUInterface {
        surface,
        _adapter: adapter,
        device,
        queue,
        swapchain_format,
        depth_texture,
        intermediate_texture,
        intermediate_texture_2,
        sky_hdr_texture,
        gpu_profiler,
    };

    return interface;
}

/// Create shader context for a shader. It creates render pipeline and bind group layout for the shader.
pub fn create_shader_context(interface: &WebGPUInterface, name: &str) -> WebGPUShaderContext {
    match name {
        "DifferedGBuffer" => {
            return differed_shading::create_differed_gbuffer_shader_context(interface);
        }
        _ => {
            return differed_shading::create_differed_gbuffer_shader_context(interface);
        }
    }
}

/// Main rendering function. It is called in each frame and updates rendering resources and executes render pass.
pub fn update_rendering_main(
    interface: &WebGPUInterface,
    scene: &engine::scene::Scene,
    shader_map: &mut std::collections::HashMap<std::string::String, WebGPUShaderContext>,
    global_resources: &mut WebGPUUniqueResources,
) {
    interface.gpu_profiler.begin_frame();

    let frame: wgpu::SurfaceTexture = interface
        .surface
        .get_current_texture()
        .expect("Failed to acquire next swap chain texture");

    let swapchain_view: wgpu::TextureView = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // intermediate_view: scene HDR (main), intermediate_view_2: scratch target for post-process passes
    let intermediate_view = interface
        .intermediate_texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let intermediate_view_2 = interface
        .intermediate_texture_2
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut main_command_encoder: wgpu::CommandEncoder =
        interface
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main command encoder"),
            });

    shadow::shadow_pass(interface, scene, &mut main_command_encoder, global_resources);

    if scene.parameters.is_use_sky_box {
        hdr_convertion_shading::hdr_convertion_pass(
            interface,
            scene,
            &mut main_command_encoder,
            global_resources,
        );
    }

    match scene.parameters.scene_shading_type {
        engine::scene::ShadingType::Forward => {
            forward_shading::forward_shading_pass(
                interface,
                scene,
                shader_map,
                &mut main_command_encoder,
                &intermediate_view,
                global_resources,
            );
        }

        engine::scene::ShadingType::Differed => {
            differed_shading::differed_shading_pass(
                interface,
                scene,
                shader_map,
                &mut main_command_encoder,
                &intermediate_view,
                global_resources,
            );
        }
    }

    if scene.parameters.is_use_sky_box {
        sky_shading::sky_pass(
            interface,
            scene,
            &mut main_command_encoder,
            global_resources,
        );
        copy_intermediate_2_to_main(interface, &mut main_command_encoder);
    }

    // [Post-process] Bloom pass
    if scene.parameters.is_use_bloom {
        bloom_shading::bloom_pass(
            interface,
            scene,
            &mut main_command_encoder,
            &intermediate_view_2,
            global_resources,
        );
        copy_intermediate_2_to_main(interface, &mut main_command_encoder);
    }

    // [Post-process] Composite pass
    if scene.parameters.is_use_composite {
        composite_shading::composite_pass(
            interface,
            scene,
            &mut main_command_encoder,
            &swapchain_view,
            global_resources,
        );
    }

    // [Overlay] Line grid UI shading
    if scene.parameters.is_use_grid {
        line_grid_shading::line_grid_pass(
            interface,
            scene,
            &mut main_command_encoder,
            &swapchain_view,
            global_resources,
        );
    }

    interface.gpu_profiler.resolve(&mut main_command_encoder);
    interface
        .queue
        .submit(std::iter::once(main_command_encoder.finish()));
    interface.gpu_profiler.request_readback();
    frame.present();
}

/// Copies the scratch HDR target (intermediate_texture_2) back into the main HDR target (intermediate_texture).
fn copy_intermediate_2_to_main(
    interface: &WebGPUInterface,
    command_encoder: &mut wgpu::CommandEncoder,
) {
    command_encoder.copy_texture_to_texture(
        interface.intermediate_texture_2.as_image_copy(),
        interface.intermediate_texture.as_image_copy(),
        interface.intermediate_texture.size(),
    );
}

// Utils --------------------------------------------------------------------------------------

/// Creates mesh rendering resource for an object. It creates vertex and index buffers, and textures if the object has materials.
pub fn create_mesh_rendering_resource(
    interface: &WebGPUInterface,
    mesh: &common::Mesh,
    materials: &Vec<engine::scene::SceneMaterial>,
) -> WebGPUMeshRenderingResource {
    let vertex_data: &Vec<common::Vertex> = &mesh.vertices;
    let index_data: &Vec<u32> = &mesh.indices;

    let vertex_buf: wgpu::Buffer =
        interface
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

    let index_buf: wgpu::Buffer =
        interface
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            });

    let index_count: u32 = index_data.len() as u32;

    let mat_idx = mesh.material.unwrap() as usize;
    let mat = materials.get(mat_idx).unwrap();
    let (base_color_texture, base_color_texture_view, base_color_texture_sampler) =
        create_and_write_texture(
            &interface.device,
            &interface.queue,
            "base color texture",
            mat.base_color_texture_size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &mat.base_color_texture,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );

    let (normal_texture, normal_texture_view, normal_texture_sampler) = create_and_write_texture(
        &interface.device,
        &interface.queue,
        "normal texture",
        mat.normal_texture_size,
        wgpu::TextureFormat::Rgba8Unorm,
        &mat.normal_texture,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );

    let (
        metallic_roughness_texture,
        metallic_roughness_texture_view,
        metallic_roughness_texture_sampler,
    ) = create_and_write_texture(
        &interface.device,
        &interface.queue,
        "metallic roughness texture",
        mat.metallic_roughness_texture_size,
        wgpu::TextureFormat::Rgba8Unorm,
        &mat.metallic_roughness_texture,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );

    return WebGPUMeshRenderingResource {
        vertex_buffer: vertex_buf,
        index_buffer: index_buf,
        index_count: index_count,
        uniform_buffer: None,
        bind_group: None,
        bind_group_2: None,
        shadow_uniform_buffer: None,
        shadow_bind_group: None,
        _base_color_texture: Some(base_color_texture),
        base_color_texture_view: Some(base_color_texture_view),
        base_color_texture_sampler: Some(base_color_texture_sampler),
        _normal_texture: Some(normal_texture),
        normal_texture_view: Some(normal_texture_view),
        normal_texture_sampler: Some(normal_texture_sampler),
        _metallic_roughness_texture: Some(metallic_roughness_texture),
        metallic_roughness_texture_view: Some(metallic_roughness_texture_view),
        metallic_roughness_texture_sampler: Some(metallic_roughness_texture_sampler),
    };
}

/// Creates a texture, writes data to it, and returns the texture, its view, and a sampler.
fn create_and_write_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    data: &[u8],
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * size[0]),
            rows_per_image: Some(size[1]),
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    (texture, view, sampler)
}
