mod widgets;

use crate::engine::scene::{CompositeViewTransform, Scene, ShadingType, ToneMappingType};
use crate::rendering::gpu_profiler::{CpuTimings, GpuPassGroup, GpuTimings};
use crate::types::Shared;
use widgets::*;

const DIRECTIONAL_AXES: [(&str, &str); 3] = [
    ("directional-range-x", "X"),
    ("directional-range-y", "Y"),
    ("directional-range-z", "Z"),
];

const AMBIENT_CHANNELS: [(&str, &str, &str, &str, usize); 4] = [
    ("ambient-range-r", "R", "0.0", "1.0", 0),
    ("ambient-range-g", "G", "0.0", "1.0", 1),
    ("ambient-range-b", "B", "0.0", "1.0", 2),
    ("ambient-range-intensity", "Intensity", "0.0", "10.0", 3),
];

const RENDER_TYPES: [(&str, ShadingType); 2] = [
    ("forward", ShadingType::Forward),
    ("differed", ShadingType::Differed),
];

const FORWARD_DISPLAY: [(&str, u8); 2] = [("render", 0), ("normal", 1)];

const BUFFER_TYPES: [(&str, u8); 7] = [
    ("render", 0),
    ("normal", 1),
    ("depth", 2),
    ("albedo", 3),
    ("metallic", 4),
    ("roughness", 5),
    ("specular", 6),
];

const TONE_MAPPINGS: [(&str, ToneMappingType); 4] = [
    ("off", ToneMappingType::Off),
    ("aces", ToneMappingType::Aces),
    ("filmic", ToneMappingType::Filmic),
    ("agx", ToneMappingType::Agx),
];

const VIEW_TRANSFORMS: [(&str, CompositeViewTransform); 3] = [
    ("standard", CompositeViewTransform::Standard),
    ("agx", CompositeViewTransform::Agx),
    ("filmic", CompositeViewTransform::Filmic),
];

pub fn create_frontend_gui(scene: &Shared<Scene>) {
    let wrapper = widgets::element("div");
    wrapper.set_id("dialog-wrapper");

    environment(&wrapper, scene);
    base_pass(&wrapper, scene);
    skybox(&wrapper, scene);
    postprocess(&wrapper, scene);
    overlay(&wrapper, scene);
    statistics(&wrapper, scene);
    performance(&wrapper);

    append(&gloo::utils::body(), &wrapper);
}

pub fn update_performance_statistics(
    average_frame_time_ms: f64,
    frames_per_second: f64,
    cpu_busy_percent: f64,
) {
    update_statistic(
        "frame-time-analytics-value",
        &format!("{average_frame_time_ms:.2} ms"),
    );
    update_statistic(
        "fps-analytics-value",
        &format!("{frames_per_second:.1} FPS"),
    );
    update_statistic(
        "cpu-busy-analytics-value",
        &format!("{cpu_busy_percent:.1}%"),
    );
}

fn update_statistic(id: &str, value: &str) {
    gloo::utils::document()
        .get_element_by_id(id)
        .expect("Performance statistic element should exist")
        .set_text_content(Some(value));
}

/// Updates GPU pass timings. `timings` is `None` until the first readback completes.
/// CPU encoding timings are shown regardless of GPU timestamp support.
pub fn update_gpu_statistics(
    supported: bool,
    timings: Option<GpuTimings>,
    cpu_timings: Option<CpuTimings>,
) {
    if !supported {
        for index in 0..GpuPassGroup::ALL.len() {
            update_statistic(&gpu_pass_row_id(index), "N/A");
        }
        update_statistic(GPU_TOTAL_ROW_ID, "N/A");
    } else {
        for index in 0..GpuPassGroup::ALL.len() {
            let text = match timings.and_then(|t| t.group_ms[index]) {
                Some(ms) => format!("{ms:.2} ms"),
                None => "-".to_string(),
            };
            update_statistic(&gpu_pass_row_id(index), &text);
        }
        let total = match timings {
            Some(t) => format!("{:.2} ms", t.total_ms),
            None => "-".to_string(),
        };
        update_statistic(GPU_TOTAL_ROW_ID, &total);
    }

    for index in 0..GpuPassGroup::ALL.len() {
        let text = match cpu_timings.and_then(|t| t.group_ms[index]) {
            Some(ms) => format!("{ms:.2} ms"),
            None => "-".to_string(),
        };
        update_statistic(&cpu_pass_row_id(index), &text);
    }
    let cpu_total = match cpu_timings {
        Some(t) => format!("{:.2} ms", t.total_ms),
        None => "-".to_string(),
    };
    update_statistic(CPU_TOTAL_ROW_ID, &cpu_total);
}

const GPU_TOTAL_ROW_ID: &str = "gpu-total-analytics-value";

fn gpu_pass_row_id(index: usize) -> String {
    format!("gpu-pass-{index}-analytics-value")
}

const CPU_TOTAL_ROW_ID: &str = "cpu-total-analytics-value";

fn cpu_pass_row_id(index: usize) -> String {
    format!("cpu-pass-{index}-analytics-value")
}

fn environment(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "environment", "Environment");

    let directional = accordion(&content, "directional", "Directional Light", true);
    for (index, &(id, label)) in DIRECTIONAL_AXES.iter().enumerate() {
        let value = scene
            .borrow()
            .parameters
            .light_parameters
            .directional_light_angle[index];
        let slider = Slider {
            id,
            label,
            min: "-1.0",
            max: "1.0",
            value,
        };
        append(
            &directional,
            &slider_row(scene, slider, move |s, v| {
                s.parameters.light_parameters.directional_light_angle[index] = v;
            }),
        );
    }
    let value = scene
        .borrow()
        .parameters
        .light_parameters
        .directional_light_intensity;
    let slider = Slider {
        id: "directional-range-intensity",
        label: "Intensity",
        min: "0.0",
        max: "10.0",
        value,
    };
    append(
        &directional,
        &slider_row(scene, slider, |s, v| {
            s.parameters.light_parameters.directional_light_intensity = v;
        }),
    );

    let ambient = accordion(&content, "ambient", "Ambient Light", true);
    for &(id, label, min, max, index) in &AMBIENT_CHANNELS {
        let value = scene
            .borrow()
            .parameters
            .light_parameters
            .ambient_light_color[index];
        let slider = Slider {
            id,
            label,
            min,
            max,
            value,
        };
        append(
            &ambient,
            &slider_row(scene, slider, move |s, v| {
                s.parameters.light_parameters.ambient_light_color[index] = v;
            }),
        );
    }
}

fn base_pass(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "basepass", "Base pass");
    let shading = scene.borrow().parameters.scene_shading_type;

    let forward_wrapper = wrapper_div("forward-wrapper", shading == ShadingType::Forward);
    let differed_wrapper = wrapper_div("differed-wrapper", shading == ShadingType::Differed);

    let render_type = select("render-type-select", &RENDER_TYPES, Some(shading));
    {
        let forward_wrapper = forward_wrapper.clone();
        let differed_wrapper = differed_wrapper.clone();
        on_select(scene, &render_type, &RENDER_TYPES, move |s, shading| {
            s.parameters.scene_shading_type = shading;
            let is_forward = shading == ShadingType::Forward;
            forward_wrapper.set_class_name(wrapper_class(is_forward));
            differed_wrapper.set_class_name(wrapper_class(!is_forward));
        });
    }
    append(&content, &widget_row("Rendering type", &[&render_type]));

    let forward_shading = select("forward-type-select", &[("phong", 0u8)], Some(0u8));
    append(
        &forward_wrapper,
        &widget_row("shading", &[&forward_shading]),
    );

    let forward_display = select("forward-display-select", &FORWARD_DISPLAY, None);
    on_select(scene, &forward_display, &FORWARD_DISPLAY, |s, v| {
        s.parameters.forward_debug_type = v;
    });
    append(&forward_wrapper, &widget_row("out", &[&forward_display]));
    append(&content, &forward_wrapper);

    let differed_shading = select("differed-type-select", &[("pbr", 0u8)], Some(0u8));
    append(
        &differed_wrapper,
        &widget_row("shading", &[&differed_shading]),
    );

    let buffer_type = select("buffer-type-select", &BUFFER_TYPES, None);
    on_select(scene, &buffer_type, &BUFFER_TYPES, |s, v| {
        s.parameters.differed_debug_type = v;
    });
    append(&differed_wrapper, &widget_row("out", &[&buffer_type]));
    append(&content, &differed_wrapper);

    let bg = scene.borrow().parameters.background_color;
    let hex = format!(
        "#{:02X}{:02X}{:02X}",
        (bg[0] * 255.0) as u32,
        (bg[1] * 255.0) as u32,
        (bg[2] * 255.0) as u32,
    );
    let clear_color = color_picker(scene, "background-color-picker", &hex, |s, value| {
        let rgb = u32::from_str_radix(value.trim_start_matches('#'), 16)
            .unwrap()
            .to_be_bytes();
        s.parameters.background_color = [
            rgb[1] as f32 / 255.0,
            rgb[2] as f32 / 255.0,
            rgb[3] as f32 / 255.0,
            1.0,
        ];
    });
    append(&content, &widget_row("Clear color", &[&clear_color]));
}

fn wrapper_class(active: bool) -> &'static str {
    if active {
        "widget-wrapper"
    } else {
        "widget-wrapper widget-wrapper-hidden"
    }
}

fn wrapper_div(id: &str, active: bool) -> web_sys::Element {
    let wrapper = div(if active {
        "widget-wrapper"
    } else {
        "widget-wrapper-hidden"
    });
    wrapper.set_id(id);
    wrapper
}

fn skybox(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "skybox", "Skybox");
    let checked = scene.borrow().parameters.is_use_sky_box;
    append(
        &content,
        &checkbox_row(scene, "Active", "skybox-active", checked, |s, v| {
            s.parameters.is_use_sky_box = v;
        }),
    );
}

fn postprocess(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "postprocess", "Postprocess");
    bloom(&content, scene);
    composite(&content, scene);
}

fn bloom(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = accordion(parent, "bloom", "Bloom", true);

    let checked = scene.borrow().parameters.is_use_bloom;
    append(
        &content,
        &checkbox_row(scene, "Active", "bloom-active", checked, |s, v| {
            s.parameters.is_use_bloom = v;
        }),
    );

    let value = scene.borrow().parameters.bloom_threshold;
    let slider = Slider {
        id: "threshold-range",
        label: "Threshold",
        min: "0.0",
        max: "2.0",
        value,
    };
    append(
        &content,
        &slider_row(scene, slider, |s, v| s.parameters.bloom_threshold = v),
    );
}

fn composite(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = accordion(parent, "composite", "Composite", true);

    let checked = scene.borrow().parameters.is_use_composite;
    append(
        &content,
        &checkbox_row(scene, "Active", "composite-active", checked, |s, v| {
            s.parameters.is_use_composite = v;
        }),
    );

    let value = scene.borrow().parameters.composite_exposure;
    let slider = Slider {
        id: "composite-exposure-range",
        label: "Exposure",
        min: "-3.0",
        max: "3.0",
        value,
    };
    append(
        &content,
        &slider_row(scene, slider, |s, v| s.parameters.composite_exposure = v),
    );

    let value = scene.borrow().parameters.composite_saturation;
    let slider = Slider {
        id: "composite-saturation-range",
        label: "Saturation",
        min: "0.0",
        max: "2.0",
        value,
    };
    append(
        &content,
        &slider_row(scene, slider, |s, v| s.parameters.composite_saturation = v),
    );

    let tone_mapping = select(
        "composite-tone-mapping-select",
        &TONE_MAPPINGS,
        Some(scene.borrow().parameters.tone_mapping_type),
    );
    on_select(scene, &tone_mapping, &TONE_MAPPINGS, |s, v| {
        s.parameters.tone_mapping_type = v;
    });
    append(
        &content,
        &widget_row("Tone Mapping", &[&value_wrapper(&tone_mapping)]),
    );

    let value = scene.borrow().parameters.composite_highlight_rolloff;
    let slider = Slider {
        id: "composite-highlight-rolloff-range",
        label: "Soft",
        min: "0.0",
        max: "1.0",
        value,
    };
    append(
        &content,
        &slider_row(scene, slider, |s, v| {
            s.parameters.composite_highlight_rolloff = v
        }),
    );

    let value = scene.borrow().parameters.composite_white_point;
    let slider = Slider {
        id: "composite-white-point-range",
        label: "White",
        min: "0.1",
        max: "4.0",
        value,
    };
    append(
        &content,
        &slider_row(scene, slider, |s, v| s.parameters.composite_white_point = v),
    );

    let view_transform = select(
        "composite-view-transform-select",
        &VIEW_TRANSFORMS,
        Some(scene.borrow().parameters.composite_view_transform),
    );
    on_select(scene, &view_transform, &VIEW_TRANSFORMS, |s, v| {
        s.parameters.composite_view_transform = v;
    });
    append(
        &content,
        &widget_row("View Trans", &[&value_wrapper(&view_transform)]),
    );

    let checked = scene.borrow().parameters.composite_use_dither;
    append(
        &content,
        &checkbox_row(
            scene,
            "Dithered",
            "composite-dither-active",
            checked,
            |s, v| {
                s.parameters.composite_use_dither = v;
            },
        ),
    );

    let checked = scene.borrow().parameters.is_use_gamma_correction;
    append(
        &content,
        &checkbox_row(
            scene,
            "Gamma",
            "gamma-correction-active",
            checked,
            |s, v| {
                s.parameters.is_use_gamma_correction = v;
            },
        ),
    );
}

fn overlay(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "overlay", "Overlay");
    let grid = accordion(&content, "grid", "Grid", true);

    let checked = scene.borrow().parameters.is_use_grid;
    append(
        &grid,
        &checkbox_row(scene, "Active", "grid-active", checked, |s, v| {
            s.parameters.is_use_grid = v;
        }),
    );
}

fn statistics(parent: &web_sys::Element, scene: &Shared<Scene>) {
    let content = dialog(parent, "analytics", "Statistics");
    let (objects, materials) = {
        let scene = scene.borrow();
        (scene.objects.len(), scene.materials.len())
    };

    append(
        &content,
        &stat_row("Objects", "objects-analytics-value", objects),
    );
    append(
        &content,
        &stat_row("Materials", "materials-analytics-value", materials),
    );
}

fn performance(parent: &web_sys::Element) {
    let content = dialog(parent, "performance", "Performance");

    append(
        &content,
        &stat_row("Frame time", "frame-time-analytics-value", 0),
    );
    append(&content, &stat_row("FPS", "fps-analytics-value", 0));
    append(
        &content,
        &stat_row("CPU busy", "cpu-busy-analytics-value", 0),
    );
    let cpu = accordion(&content, "performance-cpu", "CPU", true);
    for (index, group) in GpuPassGroup::ALL.iter().enumerate() {
        append(
            &cpu,
            &stat_row(
                &format!("CPU {}", group.label()),
                &cpu_pass_row_id(index),
                0,
            ),
        );
    }
    append(&cpu, &stat_row("CPU total", CPU_TOTAL_ROW_ID, 0));

    let gpu = accordion(&content, "performance-gpu", "GPU", true);
    for (index, group) in GpuPassGroup::ALL.iter().enumerate() {
        append(
            &gpu,
            &stat_row(
                &format!("GPU {}", group.label()),
                &gpu_pass_row_id(index),
                0,
            ),
        );
    }
    append(&gpu, &stat_row("GPU total", GPU_TOTAL_ROW_ID, 0));
}
