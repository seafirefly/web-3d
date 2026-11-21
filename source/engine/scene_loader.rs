use crate::engine;
use crate::engine::scene::Scene;
use crate::frontend;
use crate::rendering::webgpu::{self, WebGPUInterface};
use crate::types::Shared;
use std::cell::RefCell;

#[derive(Default)]
pub struct SceneLoadSlot {
    pub loading: bool,
    pub finished: Option<(
        engine::scene::SceneModel,
        Vec<engine::scene::SceneObject>,
        Vec<engine::scene::SceneMaterial>,
    )>,
}

pub fn create_mesh_rendering_resources(interface: &WebGPUInterface, scene: &Shared<Scene>) {
    let object_num: usize = scene.borrow().objects.len();
    for i in 0..object_num {
        let is_mesh: bool = scene.borrow().objects.get(i).unwrap().source_mesh.is_some();
        if is_mesh {
            let mesh_rendering_resource: Shared<webgpu::WebGPUMeshRenderingResource> = Shared::new(
                RefCell::new(webgpu::create_mesh_rendering_resource(
                    interface,
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
}

fn apply_loaded_scene(
    interface: &WebGPUInterface,
    scene: &Shared<Scene>,
    model: engine::scene::SceneModel,
    objects: Vec<engine::scene::SceneObject>,
    materials: Vec<engine::scene::SceneMaterial>,
) {
    {
        let mut scene_value = scene.borrow_mut();
        scene_value.objects = objects;
        scene_value.materials = materials;
        scene_value.parameters.scene_model = model;
    }
    engine::scene::batch_objects(scene);
    create_mesh_rendering_resources(interface, scene);

    let (object_count, material_count) = {
        let scene_value = scene.borrow();
        (scene_value.objects.len(), scene_value.materials.len())
    };
    frontend::gui::update_scene_statistics(object_count, material_count);
}

/// Applies a finished scene load, then starts loading the model requested from the GUI (if any).
/// Loading runs asynchronously so the render loop keeps running; the requested model is kept
/// pending until the in-flight load completes.
pub fn update_scene_model(
    interface: &WebGPUInterface,
    scene: &Shared<Scene>,
    load_slot: &Shared<SceneLoadSlot>,
) {
    let finished = load_slot.borrow_mut().finished.take();
    if let Some((model, objects, materials)) = finished {
        load_slot.borrow_mut().loading = false;
        apply_loaded_scene(interface, scene, model, objects, materials);
    }

    if load_slot.borrow().loading {
        return;
    }
    let Some(model) = scene.borrow_mut().parameters.requested_scene_model.take() else {
        return;
    };
    if model == scene.borrow().parameters.scene_model {
        return;
    }

    load_slot.borrow_mut().loading = true;
    let load_slot = load_slot.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let (objects, materials, _) = engine::asset_loader::load_gltf_scene(model.path()).await;
        load_slot.borrow_mut().finished = Some((model, objects, materials));
    });
}
