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
    let f: Shared<Option<_>> = Shared::new(RefCell::new(None));
    let g: Shared<Option<_>> = f.clone();
    *g.borrow_mut() = Some(wasm_bindgen::closure::Closure::wrap(Box::new(move || {
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
        request_animation_frame(f.borrow().as_ref().unwrap());
    })
        as Box<dyn FnMut()>));
    request_animation_frame(g.borrow().as_ref().unwrap());

    debug_log_with_time("Main end");
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
            web_sys::window()
                .expect("should have a Window")
                .performance()
                .expect("should have a Performance")
                .now(),
            message
        );
    }
}
