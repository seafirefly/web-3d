use crate::engine;
use crate::rendering;
use image::codecs::hdr::HdrDecoder;
use image::ImageDecoder;

// Utility

#[allow(dead_code)]
pub fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let origin = location.origin().unwrap();
    /*if !origin.ends_with("learn-wgpu") {
        origin = format!("{}/learn-wgpu", origin);
    }*/
    let base = reqwest::Url::parse(&format!("{}/", origin,)).unwrap();
    base.join(file_name).unwrap()
}

#[allow(dead_code)]
pub async fn file_status(file_name: &str) -> u16 {
    let url = format_url(file_name);
    let status = reqwest::get(url).await.unwrap().status();
    let code = status.as_u16();

    return code;
}

#[allow(dead_code)]
pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR not set"))
                .join("res")
                .join(file_name);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

#[allow(dead_code)]
pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
            //log::debug!("Load {} byte from {}", data.len(), file_name);
        } else {
            let path = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR not set"))
                .join("res")
                .join(file_name);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}

#[allow(dead_code)]
async fn extract_texture_data(file_name: &String) -> (Vec<u8>, [u32; 2]) {
    let mut out_data: Vec<u8> = Vec::new();
    let mut out_size: [u32; 2] = [1, 1];

    // assume .png or .bmp only
    let mut rgba_path: String = file_name.clone();
    if rgba_path.contains(".png") {
        rgba_path = rgba_path.replace(".png", ".rgba");
    } else if rgba_path.contains("jpg") {
        rgba_path = rgba_path.replace(".jpg", ".rgba");
    } else if rgba_path.contains("jpeg") {
        rgba_path = rgba_path.replace(".jpeg", ".rgba");
    }

    // this check is slow, so assume .rgba is exist
    /*
    let is_exist_rgba: bool = !load_string(&rgba_path.as_str())
        .await
        .unwrap()
        .starts_with("<!DOCTYPE html>");
    */
    let is_exist_rgba = true;

    // Load .rgba - ref : bin/image_convert.rs
    if is_exist_rgba {
        let texture_data = load_binary(&rgba_path.as_str())
            .await
            .expect("Failed to load texture");

        let data_width: u32 = load_4byte_to_u32(&texture_data[0..4]);
        let data_height: u32 = load_4byte_to_u32(&texture_data[4..8]);

        out_data = texture_data[8..texture_data.len()].to_vec();
        out_size = [data_width, data_height];
    }
    // Load .png slower
    /*
    else {
        let texture_data = load_binary(&png_path.as_str())
            .await
            .expect("Failed to load texture");
        let texture_image =
            image::load_from_memory_with_format(&texture_data, image::ImageFormat::Png);

        if texture_image.is_ok() {
            let texture_image_unwrap = texture_image.unwrap();
            out_data = texture_image_unwrap.to_rgba8().to_vec();
            out_size = [
                texture_image_unwrap.dimensions().0,
                texture_image_unwrap.dimensions().1,
            ];
        }
    }*/

    return (out_data, out_size);
}

#[allow(dead_code)]
fn load_4byte_to_u32(bytes: &[u8]) -> u32 {
    let out_value: u32 = ((bytes[0] as u32) << 24)
        + ((bytes[1] as u32) << 16)
        + ((bytes[2] as u32) << 8)
        + ((bytes[3] as u32) << 0);

    return out_value;
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
// public

pub async fn load_gltf_scene(
    file_name: &str,
) -> (
    Vec<engine::scene::SceneObject>,
    Vec<engine::scene::SceneMaterial>,
    engine::scene::SceneLightParameter,
) {
    let gltf_text = load_string(file_name)
        .await
        .expect("Failed to parse .gltf file path string");
    let gltf_cursor: std::io::Cursor<String> = std::io::Cursor::new(gltf_text);
    let gltf_reader: std::io::BufReader<_> = std::io::BufReader::new(gltf_cursor);

    let gltf: gltf::Gltf = gltf::Gltf::from_reader(gltf_reader).expect("Failed to read .gltf file");
    let mut buffer_data: Vec<Vec<u8>> = Vec::new();

    let slash_num: usize = file_name.rfind("/").unwrap() + 1;
    let folder_path = file_name.split_at(slash_num).0;

    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                //if let Some(blob) = gltf.blob.as_deref() {
                //    buffer_data.push(blob.into());
                //    log::debug!("Found a bin, saving");
                //};
            }
            gltf::buffer::Source::Uri(uri) => {
                let binary_path = folder_path.to_string() + uri;
                let bin = load_binary(&binary_path.as_str())
                    .await
                    .expect("Failed to load binary");
                buffer_data.push(bin);
            }
        }
    }

    let mut out_objects: Vec<engine::scene::SceneObject> = Vec::new();
    let mut out_materials: Vec<engine::scene::SceneMaterial> = Vec::new();
    let mut out_light_parameters: engine::scene::SceneLightParameter =
        engine::scene::SceneParameter::new().light_parameters;
    let mut num_node: u32 = 0;
    let mut num_verts: u32 = 0;
    let mut num_indices: u32 = 0;

    // Create scene object from meshes
    for node in gltf.nodes() {
        //log::debug!("Node : {}", node.name().unwrap());

        // Mesh node
        let mut mesh: Option<rendering::common::Mesh> = None;
        if node.mesh().is_some() {
            mesh = Some(get_gltf_mesh_from_node(&node, &buffer_data));
        }

        // Debug only
        num_node += 1;
        if mesh.is_some() {
            num_verts += mesh.as_ref().unwrap().vertices.len() as u32;
            num_indices += mesh.as_ref().unwrap().indices.len() as u32;
        }

        let mut scene_object = engine::scene::SceneObject {
            _name: Some(node.name().unwrap().to_string()),
            _shading_type: 44,
            world_transform: node.transform().matrix(),
            source_mesh: if mesh.is_some() {
                Some(std::rc::Rc::new(std::cell::RefCell::new(mesh.unwrap())))
            } else {
                None
            },
            mesh_rendering_resource: None,
            index: node.index() as u32,
            ..Default::default()
        };

        for child in node.children().into_iter() {
            scene_object.child_index.push(child.index() as u32);
        }

        out_objects.push(scene_object);
    }

    // Build parent tree
    let mut parent_vec: Vec<Option<u32>> = vec![None; out_objects.len()];
    {
        for object in out_objects.iter() {
            for child in object.child_index.iter() {
                let inner = parent_vec.get_mut(*child as usize).unwrap();
                inner.replace(object.index);
            }
        }
        for i in 0..parent_vec.len() {
            let parent = parent_vec.get(i).unwrap();
            if parent.is_some() {
                let object = out_objects.get_mut(i).unwrap();
                object.parent_index = Some(parent.unwrap());
            }
        }
    }

    // Convert object local matrix to world matrix
    let mut matrix_vec: Vec<[[f32; 4]; 4]> = Vec::with_capacity(out_objects.len());
    for object in &out_objects {
        let mut model_matrix = glam::Mat4::from_cols_array_2d(&object.world_transform);
        if object.parent_index.is_some() {
            let mut parent_index = *object.parent_index.as_ref().unwrap();
            loop {
                model_matrix = glam::Mat4::from_cols_array_2d(
                    &out_objects
                        .get(parent_index as usize)
                        .unwrap()
                        .world_transform,
                ) * model_matrix;

                let parent_option = out_objects.get(parent_index as usize).unwrap().parent_index;

                if parent_option.is_some() {
                    parent_index = parent_option.unwrap();
                    continue;
                }
                break;
            }
        }
        matrix_vec.push(model_matrix.to_cols_array_2d());
    }
    for i in 0..matrix_vec.len() {
        out_objects.get_mut(i).unwrap().world_transform = matrix_vec[i];
    }

    // Load materials
    for material in gltf.materials() {
        let scene_material: engine::scene::SceneMaterial =
            get_gltf_material(&material, &buffer_data, &folder_path).await;
        out_materials.push(scene_material);
    }

    // Extract first directional light from glTF nodes and apply its world direction.
    for node in gltf.nodes() {
        if let Some(light) = node.light() {
            if let gltf::khr_lights_punctual::Kind::Directional = light.kind() {
                let world_transform = glam::Mat4::from_cols_array_2d(
                    &out_objects.get(node.index()).unwrap().world_transform,
                );
                let rotation =
                    glam::Mat3::from_quat(world_transform.to_scale_rotation_translation().1);
                // glTF directional light points to local -Z before node rotation.
                let direction = (rotation * glam::Vec3::NEG_Z).normalize_or_zero();
                out_light_parameters.directional_light_angle = direction.to_array();

                out_light_parameters.directional_light_intensity = light.intensity() / ( 683.0); // convert from lumen to watt
                break;
            }
        }
    }

    log::debug!(
        "\n {} \n nodes : {}\n verts : {},\n tris  : {},\n mat   : {}",
        &file_name,
        num_node,
        num_verts,
        num_indices / 3,
        out_materials.len()
    );

    return (out_objects, out_materials, out_light_parameters);
}

pub async fn load_hdr_file(file_name: &str) -> (Vec<half::f16>, u32, u32) {
    let hdr_binary = load_binary(file_name)
        .await
        .expect("Failed to load HDR file");

    let reader = std::io::BufReader::new(&hdr_binary[..]);
    let decoder = HdrDecoder::new(reader).expect("Failed to decode HDR");
    let metadata = decoder.metadata();
    log::debug!(
        "HDR Metadata : width {}, height {}",
        metadata.width,
        metadata.height
    );

    // HDRピクセル（f32 * 3チャンネル）を読み込み
    let mut hdr_buffer: Vec<u8> = vec![0u8; (metadata.width * metadata.height * 3 * 4) as usize];
    decoder.read_image(&mut hdr_buffer).unwrap();

    let f32_data: &[f32] = bytemuck::cast_slice(&hdr_buffer);

    let f16_data: Vec<half::f16> = f32_data
        .into_iter()
        .map(|&value| half::f16::from_f32(value))
        .collect();

    let mut rgba_data = Vec::with_capacity((metadata.width * metadata.height * 4) as usize);

    for rgb in f16_data.chunks_exact(3) {
        rgba_data.push(rgb[0]); // R
        rgba_data.push(rgb[1]); // G
        rgba_data.push(rgb[2]); // B
        rgba_data.push(half::f16::from_f32(1.0)); // A (アルファチャンネルを追加)
    }

    return (rgba_data, metadata.width, metadata.height);
}

// private

fn get_gltf_mesh_from_node(
    node: &gltf::Node<'_>,
    buffer_data: &Vec<Vec<u8>>,
) -> rendering::common::Mesh {
    let mesh: gltf::Mesh<'_> = node.mesh().expect("Got mesh");

    let mut mesh_vertices: Vec<rendering::common::Vertex> = Vec::new();
    let mut mesh_indices: Vec<u32> = Vec::new();
    let mut mesh_material: Option<u32> = None;

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));
        if reader.read_positions().is_none() {
            continue;
        }

        let mut positions: Vec<[f32; 3]> = Vec::<[f32; 3]>::new();
        let mut normals: Vec<[f32; 3]> = Vec::<[f32; 3]>::new();
        let mut colors: Vec<[f32; 3]> = Vec::<[f32; 3]>::new();
        let mut uvs: Vec<(usize, [f32; 2])> = Vec::new();
        let mut tangents: Vec<[f32; 4]> = Vec::<[f32; 4]>::new();

        if reader.read_positions().is_some() {
            positions = {
                let iter = reader.read_positions().unwrap();
                iter.collect::<Vec<_>>()
            };
        }
        if reader.read_normals().is_some() {
            normals = {
                let iter = reader.read_normals().unwrap();
                iter.collect::<Vec<_>>()
            };
        }
        if reader.read_colors(0).is_some() {
            colors = {
                let iter = reader.read_colors(0).unwrap().into_rgb_f32();
                iter.collect::<Vec<_>>()
            };
        }
        if reader.read_tex_coords(0).is_some() {
            uvs = {
                let iter = reader.read_tex_coords(0).unwrap().into_f32();
                iter.enumerate().collect::<Vec<_>>()
            };
        }
        if reader.read_tangents().is_some() {
            tangents = {
                let iter = reader.read_tangents().unwrap();
                iter.collect::<Vec<_>>()
            }
        }

        let mut vertices: Vec<rendering::common::Vertex> = Vec::new();
        for i in 0..positions.len() {
            vertices.push(rendering::common::Vertex {
                _pos: if positions.len() > 0 {
                    [positions[i][0], positions[i][1], positions[i][2], 1.0]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                },
                _color: if colors.len() > 0 {
                    colors[i]
                } else {
                    [0.0, 0.0, 0.0]
                },
                _uv: if uvs.len() > 0 { uvs[i].1 } else { [0.0, 0.0] },
                _normal: if normals.len() > 0 {
                    [normals[i][0], normals[i][1], normals[i][2]]
                } else {
                    [0.0, 1.0, 0.0]
                },
                _tangent: if tangents.len() > 0 {
                    [tangents[i][0], tangents[i][1], tangents[i][2], tangents[i][3]]
                } else {
                    [1.0, 0.0, 0.0, 1.0]
                },
            });
        }

        let mut indices = {
            let iter = reader.read_indices().unwrap().into_u32();
            iter.collect::<Vec<_>>()
        };

        mesh_vertices.append(&mut vertices);
        mesh_indices.append(&mut indices);
        mesh_material = if primitive.material().index().is_some() {
            Some(primitive.material().index().unwrap() as u32)
        } else {
            mesh_material
        };

        /*
        log::debug!(
            "Mesh : vertice {}, indices {}, normals {}",
            mesh_vertices.len(),
            mesh_indices.len(),
            normals.len()
        );
        */
    }

    rendering::common::Mesh {
        _name: mesh.name().unwrap().to_string(),
        vertices: mesh_vertices,
        indices: mesh_indices,
        material: mesh_material,
    }
}

async fn get_gltf_material<'a>(
    material: &gltf::Material<'a>,
    buffer_data: &Vec<Vec<u8>>,
    gltf_folder_path: &str,
) -> engine::scene::SceneMaterial {
    let pbr = material.pbr_metallic_roughness();

    // base color factor
    let base_color_factor = pbr.base_color_factor();

    // base color texture
    let mut base_color_texture_data: Vec<u8> = Vec::new();
    let mut base_color_texture_size: [u32; 2] = [1, 1];
    {
        if pbr.base_color_texture().is_some() {
            let base_color_texture_source = &pbr
                .base_color_texture()
                .map(|tex| tex.texture().source().source())
                .expect("texture");

            match base_color_texture_source {
                gltf::image::Source::View { view, mime_type: _ } => {
                    // embedded data is yet
                    base_color_texture_data = buffer_data[view.buffer().index()].clone();
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => {
                    // from url
                    let texture_path = gltf_folder_path.to_string() + uri;
                    (base_color_texture_data, base_color_texture_size) =
                        extract_texture_data(&texture_path).await;
                }
            };
        }


    }

    // normal map
    let mut normal_texture_data: Vec<u8> = Vec::new();
    let mut normal_texture_size: [u32; 2] = [1, 1];
    {
        if material.normal_texture().is_some() {
            let normal_source = &material
                .normal_texture()
                .expect("Should have normal texture")
                .texture()
                .source()
                .source();
            match normal_source {
                gltf::image::Source::View { view, mime_type: _ } => {
                    // embedded data is yet
                    normal_texture_data = buffer_data[view.buffer().index()].clone();
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => {
                    // from url
                    let texture_path = gltf_folder_path.to_string() + uri;
                    (normal_texture_data, normal_texture_size) =
                        extract_texture_data(&texture_path).await;
                }
            }
        }
    }

    // metallic roughness factor
    let metallic_factor = pbr.metallic_factor();
    let roughness_factor = pbr.roughness_factor();

    // metalic roughness texture
    let mut metallic_roughness_texture_data: Vec<u8> = Vec::new();
    let mut metallic_roughness_texture_size: [u32; 2] = [1, 1];
    {
        if pbr.metallic_roughness_texture().is_some() {
            let metal_texture_source = &pbr
                .metallic_roughness_texture()
                .map(|tex| tex.texture().source().source())
                .expect("texture");

            match metal_texture_source {
                gltf::image::Source::View { view, mime_type: _ } => {
                    // embedded data is yet
                    metallic_roughness_texture_data = buffer_data[view.buffer().index()].clone();
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => {
                    // from url
                    let texture_path = gltf_folder_path.to_string() + uri;
                    (metallic_roughness_texture_data, metallic_roughness_texture_size) =
                        extract_texture_data(&texture_path).await;
                }
            };
        }
    }

    if !base_color_texture_data.is_empty() {
        for pixel in base_color_texture_data.chunks_exact_mut(4) {
            let r_linear = srgb_to_linear(pixel[0] as f32 / 255.0);
            let g_linear = srgb_to_linear(pixel[1] as f32 / 255.0);
            let b_linear = srgb_to_linear(pixel[2] as f32 / 255.0);

            let r_out = linear_to_srgb((r_linear * base_color_factor[0]).clamp(0.0, 1.0));
            let g_out = linear_to_srgb((g_linear * base_color_factor[1]).clamp(0.0, 1.0));
            let b_out = linear_to_srgb((b_linear * base_color_factor[2]).clamp(0.0, 1.0));

            pixel[0] = (r_out * 255.0).round() as u8;
            pixel[1] = (g_out * 255.0).round() as u8;
            pixel[2] = (b_out * 255.0).round() as u8;
            // Alpha uses linear transfer in glTF.
            pixel[3] = ((pixel[3] as f32 / 255.0) * base_color_factor[3])
                .clamp(0.0, 1.0)
                .mul_add(255.0, 0.0)
                .round() as u8;
        }
    }

    if !metallic_roughness_texture_data.is_empty() {
        for pixel in metallic_roughness_texture_data.chunks_exact_mut(4) {
            pixel[1] = ((pixel[1] as f32 / 255.0) * roughness_factor)
                .clamp(0.0, 1.0)
                .mul_add(255.0, 0.0)
                .round() as u8;
            pixel[2] = ((pixel[2] as f32 / 255.0) * metallic_factor)
                .clamp(0.0, 1.0)
                .mul_add(255.0, 0.0)
                .round() as u8;
        }
    }

    // KHR_materials_pbrSpecularGlossiness
    let pbr_specular_glossiness = material.pbr_specular_glossiness();
    if pbr_specular_glossiness.is_some() {
        let pbr_specular_glossiness =
            pbr_specular_glossiness.expect("Should have specular glossiness");
        // diffuse texture
        {
            if pbr_specular_glossiness.diffuse_texture().is_some() {
                let diffuse_texture_source = &pbr_specular_glossiness
                    .diffuse_texture()
                    .map(|tex| tex.texture().source().source())
                    .expect("Should have diffuse texture");

                match diffuse_texture_source {
                    gltf::image::Source::View { view, mime_type: _ } => {
                        // embedded data is yet
                        base_color_texture_data = buffer_data[view.buffer().index()].clone();
                    }
                    gltf::image::Source::Uri { uri, mime_type: _ } => {
                        // from url
                        let texture_path = gltf_folder_path.to_string() + uri;
                        (base_color_texture_data, base_color_texture_size) =
                            extract_texture_data(&texture_path).await;
                    }
                };
            }
        }
    }

    // empty texture
    if base_color_texture_data.is_empty() {
        base_color_texture_size = [1, 1];
        base_color_texture_data = vec![
            (linear_to_srgb(base_color_factor[0].clamp(0.0, 1.0)) * 255.0).round() as u8,
            (linear_to_srgb(base_color_factor[1].clamp(0.0, 1.0)) * 255.0).round() as u8,
            (linear_to_srgb(base_color_factor[2].clamp(0.0, 1.0)) * 255.0).round() as u8,
            (base_color_factor[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        ];
    }
    if normal_texture_data.is_empty() {
        normal_texture_data = [128, 128, 255, 255].to_vec();
    }
    if metallic_roughness_texture_data.is_empty() {
        metallic_roughness_texture_size = [1, 1];
        metallic_roughness_texture_data = vec![
            0,
            (roughness_factor.clamp(0.0, 1.0) * 255.0).round() as u8,
            (metallic_factor.clamp(0.0, 1.0) * 255.0).round() as u8,
            255,
        ];
    }

    engine::scene::SceneMaterial {
        _name: Some(material.name().unwrap().to_string()),
        base_color_texture: base_color_texture_data,
        base_color_texture_size: base_color_texture_size,
        normal_texture: normal_texture_data,
        normal_texture_size: normal_texture_size,
        metallic_roughness_texture: metallic_roughness_texture_data,
        metallic_roughness_texture_size,
        ..Default::default()
    }
}
