use crate::types::Shared;
use crate::{rendering, web};

// Scene world

#[derive(Clone)]
pub struct Scene {
    pub objects: Vec<SceneObject>,
    pub materials: Vec<SceneMaterial>,
    pub batched_objects: Vec<SceneObject>,
    pub parameters: SceneParameter,
}

// Scene obects

#[derive(Clone, Default)]
pub struct SceneObject {
    pub _name: Option<std::string::String>,
    pub index: u32,
    pub parent_index: Option<u32>,
    pub child_index: Vec<u32>,
    pub world_transform: [[f32; 4]; 4],
    pub source_mesh: Option<Shared<rendering::common::Mesh>>,
    pub _shading_type: u8,
    pub mesh_rendering_resource: Option<Shared<rendering::webgpu::WebGPUMeshRenderingResource>>,
}

#[derive(Clone, Default)]
pub struct SceneMaterial {
    pub _name: Option<std::string::String>,
    pub base_color_texture: Vec<u8>,
    pub base_color_texture_size: [u32; 2],

    pub normal_texture: Vec<u8>,
    pub normal_texture_size: [u32; 2],
    pub metallic_roughness_texture: Vec<u8>,
    pub metallic_roughness_texture_size: [u32; 2],
}

// Scene parameters

#[derive(Clone)]
pub struct SceneParameter {
    // camera
    pub eye_location: glam::Vec3,
    pub eye_direction: glam::Vec3,
    // light
    pub light_parameters: SceneLightParameter,
    // rendering config
    pub scene_shading_type: ShadingType,
    pub forward_debug_type: u8,
    pub differed_debug_type: u8,
    pub background_color: [f32; 4],
    // sky box
    pub is_use_sky_box: bool,
    // postprocess
    pub is_use_bloom: bool,
    pub bloom_threshold: f32,
    pub is_use_composite: bool,
    pub tone_mapping_type: ToneMappingType,
    pub is_use_gamma_correction: bool,
    pub composite_exposure: f32,
    pub composite_saturation: f32,
    pub composite_highlight_rolloff: f32,
    pub composite_white_point: f32,
    pub composite_view_transform: CompositeViewTransform,
    pub composite_use_dither: bool,
    // overlay
    pub is_use_grid: bool,
    // other config
    pub is_first_update: bool,
    pub is_convert_y_to_z: bool,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ToneMappingType {
    Off,
    Aces,
    Filmic,
    #[default]
    Agx,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum CompositeViewTransform {
    #[default]
    Standard,
    Agx,
    Filmic,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ShadingType {
    #[default]
    Forward,
    Differed,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct SceneLightParameter{
    pub directional_light_angle: [f32; 3],
    pub directional_light_intensity: f32,
    pub ambient_light_color: [f32; 4],
}

// Initializer

impl Scene {
    pub fn new() -> Self {
        Self {
            objects: [].to_vec(),
            batched_objects: [].to_vec(),
            materials: [].to_vec(),
            parameters: SceneParameter::new(),
        }
    }
}

impl SceneParameter {
    pub fn new() -> Self {
        Self {
            // camera
            eye_location: glam::Vec3 {
                x: 0.0,
                y: -2.0,
                z: 1.0,
            },
            eye_direction: glam::Vec3 {
                x: 0.0,
                y: 2.0,
                z: -1.0,
            },
            // light
            light_parameters: SceneLightParameter {
                directional_light_angle: [0.5, 1.0, -1.0],
                directional_light_intensity: 1.0,
                ambient_light_color: [0.2, 0.2, 0.2, 1.0],
            },
            // rendering
            scene_shading_type: ShadingType::Differed,
            forward_debug_type: 0,
            differed_debug_type: 0,
            background_color: [0.0, 0.0, 0.0, 1.0],
            // sky box
            is_use_sky_box: false,
            // postprocess
            is_use_bloom: false,
            bloom_threshold: 1.5,
            is_use_composite: true,
            tone_mapping_type: ToneMappingType::Off,
            is_use_gamma_correction: true,
            composite_exposure: 0.0,
            composite_saturation: 1.0,
            composite_highlight_rolloff: 0.25,
            composite_white_point: 1.0,
            composite_view_transform: CompositeViewTransform::Agx,
            composite_use_dither: true,
            // overlay
            is_use_grid: true,
            // other config
            is_first_update: false,
            is_convert_y_to_z: true,
        }
    }
}

// Scene utilities

#[allow(dead_code)]
pub fn batch_objects(scene: &Shared<Scene>) {
    scene.borrow_mut().batched_objects = [].to_vec();

    let mut batch_map: std::collections::HashMap<u32, rendering::common::Mesh> =
        std::collections::HashMap::with_capacity(scene.borrow().objects.len());

    for object in scene.borrow().objects.iter() {
        if object.source_mesh.is_some() {
            let source_mesh: &Shared<rendering::common::Mesh> =
                object.source_mesh.as_ref().unwrap();
            let material_option: Option<u32> = source_mesh.borrow().material;
            if material_option.is_some() {
                let material: &u32 = material_option.as_ref().unwrap();
                // init
                if batch_map.get(material).is_none() {
                    batch_map.insert(*material, rendering::common::Mesh::default());
                }
                // batch
                let batched_mesh = batch_map.get_mut(material).unwrap();
                let mut source_vertices = source_mesh.borrow().vertices.clone();
                let indices_offset = batched_mesh.vertices.len() as u32;
                let trans_matrix = glam::Mat4::from_cols_array_2d(&object.world_transform);
                let rotation_matrix =
                    glam::Mat3::from_quat(trans_matrix.to_scale_rotation_translation().1);
                for i in 0..source_vertices.len() {
                    let vert = glam::Vec4::from_array(source_vertices[i]._pos);
                    let transed_vert = trans_matrix.mul_vec4(vert);
                    source_vertices[i]._pos = transed_vert.to_array();
                    let norm = glam::Vec3::new(
                        source_vertices[i]._normal[0],
                        source_vertices[i]._normal[1],
                        source_vertices[i]._normal[2],
                    );
                    let transed_norm = rotation_matrix.mul_vec3(norm).normalize_or_zero();
                    source_vertices[i]._normal = transed_norm.to_array();

                    let tangent = glam::Vec3::new(
                        source_vertices[i]._tangent[0],
                        source_vertices[i]._tangent[1],
                        source_vertices[i]._tangent[2],
                    );
                    let transed_tangent = rotation_matrix.mul_vec3(tangent).normalize_or_zero();
                    source_vertices[i]._tangent[0] = transed_tangent.x;
                    source_vertices[i]._tangent[1] = transed_tangent.y;
                    source_vertices[i]._tangent[2] = transed_tangent.z;
                }
                batched_mesh.vertices.append(&mut source_vertices);

                let mut source_indices = source_mesh.borrow().indices.clone();
                for index in source_indices.iter_mut() {
                    *index += indices_offset;
                }
                batched_mesh.indices.append(&mut source_indices);
                batched_mesh.material = Some(*material);
            } else {
                // no material not create batched mesh
            }
        }
    }

    for batch_pair in batch_map {
        let batched_object = SceneObject {
            _name: Some("batched".to_string()),
            _shading_type: 44,
            world_transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
            source_mesh: Some(std::rc::Rc::new(std::cell::RefCell::new(batch_pair.1))),
            index: batch_pair.0,
            ..Default::default()
        };
        scene.borrow_mut().batched_objects.push(batched_object);
    }
}

pub fn update_camera_control(
    scene: &Shared<Scene>,
    in_control_event: &Shared<web::eventlistener::ControlResponseJs>,
) {
    let mut scene_value = scene.borrow_mut();
    let mut eye: glam::Vec3 = scene_value.parameters.eye_location;
    let mut direction: glam::Vec3 = scene_value.parameters.eye_direction;

    let mut control_event_js = in_control_event.borrow_mut();

    // Calculate eye direction (rotation)
    let on_left_click: bool = control_event_js.on_left_click;
    let on_right_click: bool = control_event_js.on_right_click;
    if on_right_click || on_left_click {
        let rotate_x_mat =
            glam::Mat3::from_rotation_z(-1.0 * control_event_js.movement_x as f32 * 0.005);
        direction = rotate_x_mat.mul_vec3(direction);

        let y_axis = glam::vec3(direction.x, direction.y, direction.z)
            .cross(glam::vec3(0.0, 0.0, 1.0))
            .normalize();
        let rotate_y_quat =
            glam::Quat::from_axis_angle(y_axis, -1.0 * control_event_js.movement_y as f32 * 0.005);
        direction = rotate_y_quat.mul_vec3(direction);
    }

    // Calculate eye location
    let on_wheel = control_event_js.on_wheel;
    let on_w = control_event_js.on_w;
    let on_s = control_event_js.on_s;
    let on_a = control_event_js.on_a;
    let on_d = control_event_js.on_d;
    if on_wheel {
        eye += -1.0 * direction.normalize() * control_event_js.wheel_delta_y as f32 * 0.005;
    }
    if on_w {
        eye += direction.normalize() * 0.05;
    }
    if on_s {
        eye -= direction.normalize() * 0.05;
    }
    if on_a {
        eye -= direction.normalize().cross(glam::Vec3::Z).normalize() * 0.05;
    }
    if on_d {
        eye += direction.normalize().cross(glam::Vec3::Z).normalize() * 0.05;
    }

    // Update
    scene_value.parameters.eye_location = eye;
    scene_value.parameters.eye_direction = direction;

    // Event context initialize
    control_event_js.on_left_click = false;
    control_event_js.on_right_click = false;
    control_event_js.on_wheel = false;
}
