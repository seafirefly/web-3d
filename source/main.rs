mod engine;
mod rendering;
mod types;
mod web;

use crate::rendering::webgpu::{self, WebGPUUniqueResources};
use crate::types::Shared;
use std::cell::RefCell;
use wasm_bindgen::JsCast;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const PERFORMANCE_SAMPLE_INTERVAL_MS: f64 = 500.0;

struct PerformanceMetrics {
    sample_started_at: f64,
    total_frame_time_ms: f64,
    frame_count: u32,
}

impl PerformanceMetrics {
    fn new(now: f64) -> Self {
        Self {
            sample_started_at: now,
            total_frame_time_ms: 0.0,
            frame_count: 0,
        }
    }

    fn record_frame(&mut self, frame_time_ms: f64, now: f64) -> bool {
        self.total_frame_time_ms += frame_time_ms;
        self.frame_count += 1;

        let sample_duration_ms = now - self.sample_started_at;
        if sample_duration_ms < PERFORMANCE_SAMPLE_INTERVAL_MS {
            return false;
        }

        let average_frame_time_ms = self.total_frame_time_ms / self.frame_count as f64;
        let frames_per_second = self.frame_count as f64 * 1000.0 / sample_duration_ms;
        let processing_load_percent = self.total_frame_time_ms * 100.0 / sample_duration_ms;
        web::gui::update_performance_statistics(
            average_frame_time_ms,
            frames_per_second,
            processing_load_percent,
        );

        self.sample_started_at = now;
        self.total_frame_time_ms = 0.0;
        self.frame_count = 0;
        true
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(main)]
pub async fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    debug_log_with_time("Main");

    // Initialize scene
    let scene: engine::scene::Scene = engine::scene::Scene::new();
    let scene: Shared<engine::scene::Scene> = Shared::new(RefCell::new(scene));

    // Load .gltf file and initialize scene objects and materials
    let (objects, materials, light_parameters) =
        engine::load::load_gltf_scene(engine::constant::GLTF_SCENE_TORUS_PATH).await;
    {
        let mut scene_value = scene.borrow_mut();
        scene_value.objects = objects;
        scene_value.materials = materials;
        scene_value.parameters.light_parameters = light_parameters;
    }

    // Batch scene objects
    engine::scene::batch_objects(&scene);

    // Initialize WebGPU interface
    let webgpu_interface: rendering::webgpu::WebGPUInterface =
        rendering::webgpu::init_interface().await;

    // Initialize mesh rendering context
    let object_num: usize = scene.borrow().objects.len();
    for i in 0..object_num {
        let is_mesh: bool = scene.borrow().objects.get(i).unwrap().source_mesh.is_some();
        if is_mesh {
            let mesh_rendering_resource: Shared<webgpu::WebGPUMeshRenderingResource> = Shared::new(
                RefCell::new(rendering::webgpu::create_mesh_rendering_resource(
                    &webgpu_interface,
                    &scene
                        .borrow()
                        .objects
                        .get(i)
                        .unwrap()
                        .source_mesh
                        .as_ref()
                        .unwrap()
                        .borrow(),
                    &scene.borrow().materials,
                )),
            );
            scene
                .borrow_mut()
                .objects
                .get_mut(i)
                .unwrap()
                .mesh_rendering_resource = Some(mesh_rendering_resource);
        }
    }

    // Global shader resources
    let mut material_shader_map: std::collections::HashMap<
        std::string::String,
        rendering::webgpu::WebGPUShaderContext,
    > = std::collections::HashMap::new();

    // Global unique resources that are shared in the whole rendering process. It is created once and shared between objects.
    let mut global_resources: WebGPUUniqueResources = WebGPUUniqueResources {
        differed_shading_resource: None,
        line_grid_shading_resource: None,
        bloom_shading_resource: None,
        composite_shading_resource: None,
        sky_shading_resource: None,
        hdr_convertion_resource: None,
        shadow_resource: None,
    };

    // Initialize control response JS object and event listener
    let control_response_js: Shared<web::eventlistener::ControlResponseJs> = Shared::new(
        RefCell::new(web::eventlistener::ControlResponseJs::default()),
    );
    web::eventlistener::add_event_listener_control(&control_response_js);

    // Create frontend gui interface
    web::gui::create_frontend_gui(&scene);

    // Rendering loop
    // Fetch the Performance object once, since looking it up from window every frame is costly.
    let performance: web_sys::Performance = web_sys::window()
        .expect("should have a Window")
        .performance()
        .expect("should have a Performance");
    let mut performance_metrics = PerformanceMetrics::new(performance.now());
    let f: Shared<Option<_>> = Shared::new(RefCell::new(None));
    let g: Shared<Option<_>> = f.clone();
    *g.borrow_mut() = Some(wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        let frame_started_at = performance.now();
        engine::scene::update_camera_control(&scene, &control_response_js);

        rendering::webgpu::update_rendering_main(
            &webgpu_interface,
            &scene.borrow(),
            &mut material_shader_map,
            &mut global_resources,
        );

        if scene.borrow().parameters.is_first_update {
            scene.borrow_mut().parameters.is_first_update = false;
            debug_log_with_time("Render OK");
        }
        let frame_finished_at = performance.now();
        if performance_metrics.record_frame(frame_finished_at - frame_started_at, frame_finished_at)
        {
            web::gui::update_gpu_statistics(
                webgpu_interface.gpu_profiler.is_supported(),
                webgpu_interface.gpu_profiler.latest_timings(),
            );
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    })
        as Box<dyn FnMut()>));
    request_animation_frame(g.borrow().as_ref().unwrap());

    debug_log_with_time("Main end");
}

fn performance_now() -> f64 {
    web_sys::window()
        .expect("should have a Window")
        .performance()
        .expect("should have a Performance")
        .now()
}

fn request_animation_frame(f: &wasm_bindgen::closure::Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

pub fn debug_log_with_time(message: &str) {
    let enable_log = true;

    if enable_log {
        log::debug!(
            "{:.2} ms : {}",
            performance_now(),
            message
        );
    }
}
