use crate::engine;
use crate::types::Shared;
use wasm_bindgen::JsCast;

fn document() -> web_sys::Document {
    gloo::utils::document()
}

macro_rules! widget_row {
    ($label_text:expr, $($child:expr),+ $(,)?) => {{
        let (widget_row, widget_value) = create_widget_row($label_text);
        $(widget_value.append_child($child.as_ref()).unwrap();)+
        widget_row
    }};
}

fn create_widget_row(label_text: &str) -> (web_sys::Element, web_sys::Element) {
    let widget_row: web_sys::Element = document().create_element("div").unwrap();
    widget_row.set_class_name("widget-row");

    let widget_label: web_sys::Element = document().create_element("div").unwrap();
    widget_label.set_class_name("widget-label");
    widget_label.set_text_content(Some(label_text));

    let widget_value: web_sys::Element = document().create_element("div").unwrap();
    widget_value.set_class_name("widget-value");

    widget_row.append_child(&widget_label).unwrap();
    widget_row.append_child(&widget_value).unwrap();

    (widget_row, widget_value)
}

fn create_range_input(
    input_id: &str,
    value: f32,
    min: &str,
    max: &str,
    step: &str,
) -> web_sys::HtmlInputElement {
    let input_element: web_sys::Element = document().create_element("input").unwrap();
    let input_element: web_sys::HtmlInputElement = input_element.dyn_into().unwrap();
    input_element.set_id(input_id);
    input_element.set_class_name("range-element");
    input_element.set_attribute("type", "range").unwrap();
    input_element.set_attribute("min", min).unwrap();
    input_element.set_attribute("max", max).unwrap();
    input_element.set_attribute("step", step).unwrap();

    let value_string: String = value.to_string();
    input_element.set_value(&value_string);

    input_element
}

fn create_range_value_text(text_id: &str, value: f32) -> web_sys::Element {
    let text_element: web_sys::Element = document().create_element("div").unwrap();
    text_element.set_id(text_id);
    text_element.set_class_name("range-text-element");

    let value_string: String = value.to_string();
    text_element.set_text_content(Some(&value_string));

    text_element
}

fn create_checkbox_input(input_id: &str, checked: bool) -> web_sys::HtmlInputElement {
    let input_element: web_sys::Element = document().create_element("input").unwrap();
    let input_element: web_sys::HtmlInputElement = input_element.dyn_into().unwrap();
    input_element.set_id(input_id);
    input_element.set_class_name("checkbox-element");
    input_element.set_attribute("type", "checkbox").unwrap();
    input_element.set_checked(checked);

    input_element
}

fn create_select_input(
    input_id: &str,
    options: &[(&str, &str)],
    selected_value: Option<&str>,
) -> web_sys::HtmlSelectElement {
    let select_element: web_sys::Element = document().create_element("select").unwrap();
    let select_element: web_sys::HtmlSelectElement = select_element.dyn_into().unwrap();
    select_element.set_id(input_id);
    select_element.set_class_name("select-element");

    for (label, value) in options {
        let option_element: web_sys::Element = document().create_element("option").unwrap();
        option_element.set_text_content(Some(label));
        option_element.set_attribute("value", value).unwrap();

        if selected_value == Some(*value) {
            option_element.set_attribute("selected", "").unwrap();
        }

        select_element.append_child(&option_element).unwrap();
    }

    select_element
}

fn create_accordion_section(
    root_id: &str,
    accordion_id: &str,
    label_text: &str,
    label_class_name: &str,
    content_class_name: &str,
) -> (web_sys::Element, web_sys::HtmlInputElement, web_sys::Element, web_sys::Element) {
    let root_element: web_sys::Element = document().create_element("div").unwrap();
    root_element.set_id(root_id);
    root_element.set_class_name("dialog-element dialog-element-display");

    let accordion_input_element: web_sys::Element = document().create_element("input").unwrap();
    let accordion_input_element: web_sys::HtmlInputElement = accordion_input_element.dyn_into().unwrap();
    accordion_input_element.set_attribute("type", "checkbox").unwrap();
    accordion_input_element.set_class_name("accordion-input");
    accordion_input_element.set_id(accordion_id);

    let accordion_label_element: web_sys::Element = document().create_element("label").unwrap();
    accordion_label_element.set_class_name(label_class_name);
    accordion_label_element.set_text_content(Some(label_text));
    accordion_label_element.set_attribute("for", accordion_id).unwrap();

    let accordion_content_element: web_sys::Element = document().create_element("div").unwrap();
    accordion_content_element.set_class_name(content_class_name);

    (root_element, accordion_input_element, accordion_label_element, accordion_content_element)
}

// Initialize frontend GUI

pub fn create_frontend_gui(scene: &Shared<engine::scene::Scene>) {
    create_debug_dialog(scene);
}

fn create_debug_dialog(scene: &Shared<engine::scene::Scene>) {
    let body: web_sys::HtmlElement = gloo::utils::body();

    let dialog_wrapper: web_sys::Element = gloo::utils::document().create_element("div").unwrap();
    dialog_wrapper.set_id("dialog-wrapper");

    // Environment dialog
    create_debug_dialog_environment(&dialog_wrapper, &scene);

    // Base pass dialog
    create_debug_dialog_base_pass(&dialog_wrapper, scene);

    // Sky box dialog
    create_debug_dialog_sky_box(&dialog_wrapper, scene);

    // Postprocess dialog
    create_debug_dialog_postprocess(&dialog_wrapper, &scene);

    // Overlay dialog
    create_debug_dialog_overlay(&dialog_wrapper, &scene);

    // Statistics dialog
    create_debug_dialog_statistics(&dialog_wrapper, &scene);

    body.append_child(&dialog_wrapper).unwrap();
}

// Create and append debug dialog

fn create_debug_dialog_environment(
    parent: &web_sys::Element,
    scene: &Shared<engine::scene::Scene>,
) {
    let scene_value = scene.borrow();

    let (environment_dialog, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-environment",
            "accordion-environment",
            "Environment",
            "accordion-label",
            "accordion-content",
        );

    // directional light
    {
        let directional_accordion_input_element =
            gloo::utils::document().create_element("input").unwrap();
        let directional_accordion_input_element: web_sys::HtmlInputElement =
            directional_accordion_input_element.dyn_into().unwrap();
        directional_accordion_input_element
            .set_attribute("type", "checkbox")
            .unwrap();
        directional_accordion_input_element.set_class_name("accordion-input");
        directional_accordion_input_element.set_id("accordion-directional");
        //directional_accordion_input_element.set_checked(true);

        let directional_accordion_label_element =
            gloo::utils::document().create_element("label").unwrap();
        directional_accordion_label_element.set_class_name("accordion-label inner-accordion-label");
        directional_accordion_label_element.set_text_content(Some("Directional Light"));
        directional_accordion_label_element
            .set_attribute("for", "accordion-directional")
            .unwrap();

        let directional_accordion_content_element =
            gloo::utils::document().create_element("div").unwrap();
        directional_accordion_content_element
            .set_class_name("accordion-content inner-accordion-content");

        // X
        {
            let directional_x_input_range = create_range_input(
                "directional-range-x",
                scene_value.parameters.light_parameters.directional_light_angle[0],
                "-1.0",
                "1.0",
                "0.01",
            );
            let directional_x_input_range_text = create_range_value_text(
                "directional-range-x-text",
                scene_value.parameters.light_parameters.directional_light_angle[0],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let directional_range_x_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_x_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("directional-range-x")
                                .unwrap();
                            let range_x_element: web_sys::HtmlInputElement =
                                range_x_element.dyn_into().unwrap();
                            let value: String = range_x_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.directional_light_angle[0] =
                                value.parse::<f32>().unwrap();

                            let range_x_text_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("directional-range-x-text")
                                    .unwrap();
                            range_x_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                directional_x_input_range
                    .add_event_listener_with_callback(
                        "input",
                        directional_range_x_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                directional_range_x_closure.forget();
            }

                let directional_x_element = widget_row!(
                    "X",
                    &directional_x_input_range,
                    &directional_x_input_range_text
                );

            directional_accordion_content_element
                .append_child(&directional_x_element)
                .unwrap();
        }
        // Y
        {
            let directional_y_input_range = create_range_input(
                "directional-range-y",
                scene_value.parameters.light_parameters.directional_light_angle[1],
                "-1.0",
                "1.0",
                "0.01",
            );
            let directional_y_input_range_text = create_range_value_text(
                "directional-range-y-text",
                scene_value.parameters.light_parameters.directional_light_angle[1],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let directional_range_y_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_x_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("directional-range-y")
                                .unwrap();
                            let range_y_element: web_sys::HtmlInputElement =
                                range_x_element.dyn_into().unwrap();
                            let value: String = range_y_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.directional_light_angle[1] =
                                value.parse::<f32>().unwrap();

                            let range_y_text_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("directional-range-y-text")
                                    .unwrap();
                            range_y_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                directional_y_input_range
                    .add_event_listener_with_callback(
                        "input",
                        directional_range_y_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                directional_range_y_closure.forget();
            }

                let directional_y_element = widget_row!(
                    "Y",
                    &directional_y_input_range,
                    &directional_y_input_range_text
                );

            directional_accordion_content_element
                .append_child(&directional_y_element)
                .unwrap();
        }
        // Z
        {
            let directional_z_input_range = create_range_input(
                "directional-range-z",
                scene_value.parameters.light_parameters.directional_light_angle[2],
                "-1.0",
                "1.0",
                "0.01",
            );
            let directional_z_input_range_text = create_range_value_text(
                "directional-range-z-text",
                scene_value.parameters.light_parameters.directional_light_angle[2],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let directional_range_z_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_z_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("directional-range-z")
                                .unwrap();
                            let range_z_element: web_sys::HtmlInputElement =
                                range_z_element.dyn_into().unwrap();
                            let value: String = range_z_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.directional_light_angle[2] =
                                value.parse::<f32>().unwrap();

                            let range_z_text_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("directional-range-z-text")
                                    .unwrap();
                            range_z_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                directional_z_input_range
                    .add_event_listener_with_callback(
                        "input",
                        directional_range_z_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                directional_range_z_closure.forget();
            }

                let directional_z_element = widget_row!(
                    "Z",
                    &directional_z_input_range,
                    &directional_z_input_range_text
                );

            directional_accordion_content_element
                .append_child(&directional_z_element)
                .unwrap();
        }

        // Intensity
        {
            let directional_intensity_input_range = create_range_input(
                "directional-range-intensity",
                scene_value.parameters.light_parameters.directional_light_intensity,
                "0.0",
                "10.0",
                "0.01",
            );
            let directional_intensity_input_range_text = create_range_value_text(
                "directional-range-intensity-text",
                scene_value.parameters.light_parameters.directional_light_intensity,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let directional_range_intensity_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_intensity_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("directional-range-intensity")
                                    .unwrap();
                            let range_intensity_element: web_sys::HtmlInputElement =
                                range_intensity_element.dyn_into().unwrap();
                            let value: String = range_intensity_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.directional_light_intensity =
                                value.parse::<f32>().unwrap();

                            let range_intensity_text_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("directional-range-intensity-text")
                                    .unwrap();
                            range_intensity_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                directional_intensity_input_range
                    .add_event_listener_with_callback(
                        "input",
                        directional_range_intensity_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                directional_range_intensity_closure.forget();
            }

                let directional_intensity_element = widget_row!(
                    "Intensity",
                    &directional_intensity_input_range,
                    &directional_intensity_input_range_text
                );

            directional_accordion_content_element
                .append_child(&directional_intensity_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&directional_accordion_input_element)
            .unwrap();
        accordion_content_element
            .append_child(&directional_accordion_label_element)
            .unwrap();
        accordion_content_element
            .append_child(&directional_accordion_content_element)
            .unwrap();
    }

    // ambient light
    {
        let ambient_accordion_input_element =
            gloo::utils::document().create_element("input").unwrap();
        let ambient_accordion_input_element: web_sys::HtmlInputElement =
            ambient_accordion_input_element.dyn_into().unwrap();
        ambient_accordion_input_element
            .set_attribute("type", "checkbox")
            .unwrap();
        ambient_accordion_input_element.set_class_name("accordion-input");
        ambient_accordion_input_element.set_id("accordion-ambient");

        let ambient_accordion_label_element =
            gloo::utils::document().create_element("label").unwrap();
        ambient_accordion_label_element.set_class_name("accordion-label inner-accordion-label");
        ambient_accordion_label_element.set_text_content(Some("Ambient Light"));
        ambient_accordion_label_element
            .set_attribute("for", "accordion-ambient")
            .unwrap();

        let ambient_accordion_content_element =
            gloo::utils::document().create_element("div").unwrap();
        ambient_accordion_content_element
            .set_class_name("accordion-content inner-accordion-content");

        // R
        {
            let ambient_r_input_range = create_range_input(
                "ambient-range-r",
                scene_value.parameters.light_parameters.ambient_light_color[0],
                "0.0",
                "1.0",
                "0.01",
            );
            let ambient_r_input_range_text = create_range_value_text(
                "ambient-range-r-text",
                scene_value.parameters.light_parameters.ambient_light_color[0],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let ambient_range_r_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_r_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-r")
                                .unwrap();
                            let range_r_element: web_sys::HtmlInputElement =
                                range_r_element.dyn_into().unwrap();
                            let value: String = range_r_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.ambient_light_color[0] =
                                value.parse::<f32>().unwrap();

                            let range_r_text_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-r-text")
                                .unwrap();
                            range_r_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                ambient_r_input_range
                    .add_event_listener_with_callback(
                        "input",
                        ambient_range_r_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                ambient_range_r_closure.forget();
            }

            let ambient_r_element =
                widget_row!("R", &ambient_r_input_range, &ambient_r_input_range_text);

            ambient_accordion_content_element
                .append_child(&ambient_r_element)
                .unwrap();
        }

        // G
        {
            let ambient_g_input_range = create_range_input(
                "ambient-range-g",
                scene_value.parameters.light_parameters.ambient_light_color[1],
                "0.0",
                "1.0",
                "0.01",
            );
            let ambient_g_input_range_text = create_range_value_text(
                "ambient-range-g-text",
                scene_value.parameters.light_parameters.ambient_light_color[1],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let ambient_range_g_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_g_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-g")
                                .unwrap();
                            let range_g_element: web_sys::HtmlInputElement =
                                range_g_element.dyn_into().unwrap();
                            let value: String = range_g_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.ambient_light_color[1] =
                                value.parse::<f32>().unwrap();

                            let range_g_text_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-g-text")
                                .unwrap();
                            range_g_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                ambient_g_input_range
                    .add_event_listener_with_callback(
                        "input",
                        ambient_range_g_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                ambient_range_g_closure.forget();
            }

            let ambient_g_element =
                widget_row!("G", &ambient_g_input_range, &ambient_g_input_range_text);

            ambient_accordion_content_element
                .append_child(&ambient_g_element)
                .unwrap();
        }

        // B
        {
            let ambient_b_input_range = create_range_input(
                "ambient-range-b",
                scene_value.parameters.light_parameters.ambient_light_color[2],
                "0.0",
                "1.0",
                "0.01",
            );
            let ambient_b_input_range_text = create_range_value_text(
                "ambient-range-b-text",
                scene_value.parameters.light_parameters.ambient_light_color[2],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let ambient_range_b_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_b_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-b")
                                .unwrap();
                            let range_b_element: web_sys::HtmlInputElement =
                                range_b_element.dyn_into().unwrap();
                            let value: String = range_b_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.ambient_light_color[2] =
                                value.parse::<f32>().unwrap();

                            let range_b_text_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-b-text")
                                .unwrap();
                            range_b_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                ambient_b_input_range
                    .add_event_listener_with_callback(
                        "input",
                        ambient_range_b_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                ambient_range_b_closure.forget();
            }

            let ambient_b_element =
                widget_row!("B", &ambient_b_input_range, &ambient_b_input_range_text);

            ambient_accordion_content_element
                .append_child(&ambient_b_element)
                .unwrap();
        }

        // Intensity
        {
            let ambient_intensity_input_range = create_range_input(
                "ambient-range-intensity",
                scene_value.parameters.light_parameters.ambient_light_color[3],
                "0.0",
                "10.0",
                "0.01",
            );
            let ambient_intensity_input_range_text = create_range_value_text(
                "ambient-range-intensity-text",
                scene_value.parameters.light_parameters.ambient_light_color[3],
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let ambient_range_intensity_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_intensity_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-intensity")
                                .unwrap();
                            let range_intensity_element: web_sys::HtmlInputElement =
                                range_intensity_element.dyn_into().unwrap();
                            let value: String = range_intensity_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.light_parameters.ambient_light_color[3] =
                                value.parse::<f32>().unwrap();

                            let range_intensity_text_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("ambient-range-intensity-text")
                                .unwrap();
                            range_intensity_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                ambient_intensity_input_range
                    .add_event_listener_with_callback(
                        "input",
                        ambient_range_intensity_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                ambient_range_intensity_closure.forget();
            }

            let ambient_intensity_element = widget_row!(
                "Intensity",
                &ambient_intensity_input_range,
                &ambient_intensity_input_range_text,
            );

            ambient_accordion_content_element
                .append_child(&ambient_intensity_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&ambient_accordion_input_element)
            .unwrap();
        accordion_content_element
            .append_child(&ambient_accordion_label_element)
            .unwrap();
        accordion_content_element
            .append_child(&ambient_accordion_content_element)
            .unwrap();
    }

    environment_dialog
        .append_child(&accordion_input_element)
        .unwrap();
    environment_dialog
        .append_child(&accordion_label_element)
        .unwrap();
    environment_dialog
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&environment_dialog).unwrap();
}

fn create_debug_dialog_base_pass(parent: &web_sys::Element, scene: &Shared<engine::scene::Scene>) {
    let scene_value = scene.borrow();

    let (dialog_base_pass, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-basepass",
            "accordion-basepass",
            "Base pass",
            "accordion-label",
            "accordion-content",
        );

    // rendering type
    {
        let render_type_select_element = create_select_input(
            "render-type-select",
            &[("forward", "forward"), ("differed", "differed")],
            Some(match &scene_value.parameters.scene_shading_type {
                engine::scene::ShadingType::Forward => "forward",
                engine::scene::ShadingType::Differed => "differed",
            }),
        );

        {
            let scene_clone: Shared<engine::scene::Scene> = scene.clone();

            let render_type_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::InputEvent| {
                    let render_type_element: web_sys::Element = gloo::utils::document()
                        .get_element_by_id("render-type-select")
                        .unwrap();
                    let render_type_element: web_sys::HtmlSelectElement =
                        render_type_element.dyn_into().unwrap();
                    let value: String = render_type_element.value();

                    let forward_wrapper: web_sys::Element = gloo::utils::document()
                        .get_element_by_id("forward-wrapper")
                        .unwrap();
                    let forward_wrapper: web_sys::HtmlElement = forward_wrapper.dyn_into().unwrap();

                    let differed_wrapper: web_sys::Element = gloo::utils::document()
                        .get_element_by_id("differed-wrapper")
                        .unwrap();
                    let differed_wrapper: web_sys::HtmlElement =
                        differed_wrapper.dyn_into().unwrap();

                    let mut scene_value = scene_clone.borrow_mut();
                    match value.as_str() {
                        "forward" => {
                            scene_value.parameters.scene_shading_type =
                                engine::scene::ShadingType::Forward;

                            forward_wrapper.set_class_name("widget-wrapper");
                            differed_wrapper.set_class_name("widget-wrapper widget-wrapper-hidden");
                        }
                        "differed" => {
                            scene_value.parameters.scene_shading_type =
                                engine::scene::ShadingType::Differed;

                            forward_wrapper.set_class_name("widget-wrapper widget-wrapper-hidden");
                            differed_wrapper.set_class_name("widget-wrapper");
                        }
                        _ => {}
                    }
                }) as Box<dyn FnMut(_)>);

            render_type_select_element
                .add_event_listener_with_callback(
                    "change",
                    render_type_closure.as_ref().unchecked_ref(),
                )
                .unwrap();
            render_type_closure.forget();
        }

            let render_type_element = widget_row!("Rendering type", &render_type_select_element);

        accordion_content_element
            .append_child(&render_type_element)
            .unwrap();
    }

    // forward wrapper
    {
        let forward_wrapper = gloo::utils::document().create_element("div").unwrap();
        forward_wrapper.set_id("forward-wrapper");
        forward_wrapper.set_class_name("widget-wrapper");
        if scene.borrow().parameters.scene_shading_type != engine::scene::ShadingType::Forward {
            forward_wrapper.set_class_name("widget-wrapper-hidden");
        }

        // shader type
        {
            let shader_type_select_element =
                create_select_input("forward-type-select", &[("phong", "phong")], Some("phong"));

            /*
            {
            let shader_type_element = widget_row!("shading", &shader_type_select_element);
                                .get_element_by_id("buffer-type-select")
                                .unwrap();
                            let buffer_type_element: web_sys::HtmlSelectElement =
                                buffer_type_element.dyn_into().unwrap();
                            let value: String = buffer_type_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            match value.as_str() {
                                "render" => scene_value.variables.differed_debug_type = 0,
                                "normal" => scene_value.variables.differed_debug_type = 1,
                                "depth" => scene_value.variables.differed_debug_type = 2,
                                "albedo" => scene_value.variables.differed_debug_type = 3,
                                "metallic" => scene_value.variables.differed_debug_type = 4,
                                _ => scene_value.variables.differed_debug_type = 0,
                            }
                        },
                    )
                        as Box<dyn FnMut(_)>);

                shader_type_select_element
                    .add_event_listener_with_callback(
                        "change",
                        buffer_type_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                buffer_type_closure.forget();
            }*/

                let shader_type_element = widget_row!("shading", &shader_type_select_element);

            forward_wrapper.append_child(&shader_type_element).unwrap();
        }

        // display
        {
            let forward_display_select_element =
                create_select_input("forward-display-select", &[("render", "render"), ("normal", "normal")], None);

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let forward_display_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let forward_display_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("forward-display-select")
                                .unwrap();
                            let forward_display_element: web_sys::HtmlSelectElement =
                                forward_display_element.dyn_into().unwrap();
                            let value: String = forward_display_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            match value.as_str() {
                                "rendering" => scene_value.parameters.forward_debug_type = 0,
                                "normal" => scene_value.parameters.forward_debug_type = 1,
                                _ => scene_value.parameters.forward_debug_type = 0,
                            }
                        },
                    )
                        as Box<dyn FnMut(_)>);

                forward_display_select_element
                    .add_event_listener_with_callback(
                        "change",
                        forward_display_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                forward_display_closure.forget();
            }

                let forward_display_element = widget_row!("out", &forward_display_select_element);

            forward_wrapper
                .append_child(&forward_display_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&forward_wrapper)
            .unwrap();
    }

    // differed wrapper
    {
        let differed_wrapper = gloo::utils::document().create_element("div").unwrap();
        differed_wrapper.set_id("differed-wrapper");
        differed_wrapper.set_class_name("widget-wrapper");
        if scene.borrow().parameters.scene_shading_type != engine::scene::ShadingType::Differed {
            differed_wrapper.set_class_name("widget-wrapper-hidden");
        }

        // shader type
        {
            let shader_type_select_element =
                create_select_input("differed-type-select", &[("pbr", "pbr")], Some("pbr"));

            /*
            {
            let shader_type_element = widget_row!("shading", &shader_type_select_element);
                                .get_element_by_id("buffer-type-select")
                                .unwrap();
                            let buffer_type_element: web_sys::HtmlSelectElement =
                                buffer_type_element.dyn_into().unwrap();
                            let value: String = buffer_type_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            match value.as_str() {
                                "render" => scene_value.variables.differed_debug_type = 0,
                                "normal" => scene_value.variables.differed_debug_type = 1,
                                "depth" => scene_value.variables.differed_debug_type = 2,
                                "albedo" => scene_value.variables.differed_debug_type = 3,
                                "metallic" => scene_value.variables.differed_debug_type = 4,
                                _ => scene_value.variables.differed_debug_type = 0,
                            }
                        },
                    )
                        as Box<dyn FnMut(_)>);

                shader_type_select_element
                    .add_event_listener_with_callback(
                        "change",
                        buffer_type_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                buffer_type_closure.forget();
            }*/

                let shader_type_element = widget_row!("shading", &shader_type_select_element);

            differed_wrapper.append_child(&shader_type_element).unwrap();
        }

        // buffer
        {
            let buffer_type_select_element = create_select_input(
                "buffer-type-select",
                &[
                    ("render", "render"),
                    ("normal", "normal"),
                    ("depth", "depth"),
                    ("albedo", "albedo"),
                    ("metallic", "metallic"),
                    ("roughness", "roughness"),
                    ("specular", "specular"),
                ],
                None,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let buffer_type_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let buffer_type_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("buffer-type-select")
                                .unwrap();
                            let buffer_type_element: web_sys::HtmlSelectElement =
                                buffer_type_element.dyn_into().unwrap();
                            let value: String = buffer_type_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            match value.as_str() {
                                "render" => scene_value.parameters.differed_debug_type = 0,
                                "normal" => scene_value.parameters.differed_debug_type = 1,
                                "depth" => scene_value.parameters.differed_debug_type = 2,
                                "albedo" => scene_value.parameters.differed_debug_type = 3,
                                "metallic" => scene_value.parameters.differed_debug_type = 4,
                                "roughness" => scene_value.parameters.differed_debug_type = 5,
                                "specular" => scene_value.parameters.differed_debug_type = 6,
                                _ => scene_value.parameters.differed_debug_type = 0,
                            }
                        },
                    ) as Box<dyn FnMut(_)>);

                buffer_type_select_element
                    .add_event_listener_with_callback(
                        "change",
                        buffer_type_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                buffer_type_closure.forget();
            }

            let buffer_type_element = widget_row!("out", &buffer_type_select_element);

            differed_wrapper.append_child(&buffer_type_element).unwrap();
        }

        accordion_content_element
            .append_child(&differed_wrapper)
            .unwrap();
    }

    // clear color
    {
        let clearcolor_picker_element: web_sys::Element =
            gloo::utils::document().create_element("input").unwrap();
        clearcolor_picker_element.set_class_name("widget-value color-picker-element");
        clearcolor_picker_element.set_id("background-color-picker");
        clearcolor_picker_element
            .set_attribute("type", "color")
            .unwrap();
        {
            let bg_color: [f32; 4] = scene_value.parameters.background_color;
            let r_uint: u32 = (bg_color[0] * 255.0) as u32;
            let r_hex: String = format!("{r_uint:X}");
            let g_uint: u32 = (bg_color[1] * 255.0) as u32;
            let g_hex: String = format!("{g_uint:X}");
            let b_uint: u32 = (bg_color[2] * 255.0) as u32;
            let b_hex: String = format!("{b_uint:X}");

            let hex_string: String = "#".to_string() + &r_hex + &g_hex + &b_hex;
            clearcolor_picker_element
                .set_attribute("value", &hex_string)
                .unwrap();
        }

        let clearcolor_element = widget_row!("Clear color", &clearcolor_picker_element);

        {
            let scene_clone: Shared<engine::scene::Scene> = scene.clone();

            let bgcolor_picker_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::InputEvent| {
                    let picker_element: web_sys::Element = gloo::utils::document()
                        .get_element_by_id("background-color-picker")
                        .unwrap();
                    let picker_element: web_sys::HtmlInputElement =
                        picker_element.dyn_into().unwrap();
                    let value: String = picker_element.value();

                    let color_hex = value.trim_start_matches("#");
                    let color_u8: [u8; 4] =
                        u32::from_str_radix(&color_hex, 16).unwrap().to_be_bytes();

                    let mut scene_value = scene_clone.borrow_mut();
                    scene_value.parameters.background_color = [
                        color_u8[1] as f32 / 256 as f32,
                        color_u8[2] as f32 / 256 as f32,
                        color_u8[3] as f32 / 256 as f32,
                        1.0,
                    ];
                }) as Box<dyn FnMut(_)>);

            clearcolor_picker_element
                .add_event_listener_with_callback(
                    "input",
                    bgcolor_picker_closure.as_ref().unchecked_ref(),
                )
                .unwrap();
            bgcolor_picker_closure.forget();
        }

        accordion_content_element
            .append_child(&clearcolor_element)
            .unwrap();
    }

    dialog_base_pass
        .append_child(&accordion_input_element)
        .unwrap();
    dialog_base_pass
        .append_child(&accordion_label_element)
        .unwrap();
    dialog_base_pass
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&dialog_base_pass).unwrap();
}

fn create_debug_dialog_sky_box(parent: &web_sys::Element, scene: &Shared<engine::scene::Scene>) {
    let (view_statistics, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-skybox",
            "accordion-skybox",
            "Skybox",
            "accordion-label",
            "accordion-content",
        );

    // active
    {
        let active_content_element = gloo::utils::document().create_element("div").unwrap();
        active_content_element.set_class_name("widget-value");
        active_content_element.set_id("active-analytics-value");

        {
            let sky_box_active_input_checkbox =
                create_checkbox_input("skybox-active", scene.borrow().parameters.is_use_sky_box);

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let sky_box_active_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let sky_box_active_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("skybox-active")
                                .unwrap();
                            let sky_box_active_element: web_sys::HtmlInputElement =
                                sky_box_active_element.dyn_into().unwrap();
                            let value: bool = sky_box_active_element.checked();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.is_use_sky_box = value;
                        },
                    )
                        as Box<dyn FnMut(_)>);

                sky_box_active_input_checkbox
                    .add_event_listener_with_callback(
                        "input",
                        sky_box_active_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                sky_box_active_closure.forget();
            }

            active_content_element
                .append_child(&sky_box_active_input_checkbox)
                .unwrap();
        }

        let active_element = widget_row!("Active", &active_content_element);

        accordion_content_element
            .append_child(&active_element)
            .unwrap();
    }

    view_statistics
        .append_child(&accordion_input_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_label_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&view_statistics).unwrap();
}

fn create_debug_dialog_postprocess(
    parent: &web_sys::Element,
    scene: &Shared<engine::scene::Scene>,
) {
    let scene_value = scene.borrow();

    let (view_statistics, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-postprocess",
            "accordion-postprocess",
            "Postprocess",
            "accordion-label",
            "accordion-content",
        );

    // bloom
    {
        let bloom_accordion_input_element =
            gloo::utils::document().create_element("input").unwrap();
        let bloom_accordion_input_element: web_sys::HtmlInputElement =
            bloom_accordion_input_element.dyn_into().unwrap();
        bloom_accordion_input_element
            .set_attribute("type", "checkbox")
            .unwrap();
        bloom_accordion_input_element.set_class_name("accordion-input");
        bloom_accordion_input_element.set_id("accordion-bloom");
        //bloom_accordion_input_element.set_checked(true);

        let bloom_accordion_label_element =
            gloo::utils::document().create_element("label").unwrap();
        bloom_accordion_label_element.set_class_name("accordion-label inner-accordion-label");
        bloom_accordion_label_element.set_text_content(Some("Bloom"));
        bloom_accordion_label_element
            .set_attribute("for", "accordion-bloom")
            .unwrap();

        let bloom_accordion_content_element =
            gloo::utils::document().create_element("div").unwrap();
        bloom_accordion_content_element.set_class_name("accordion-content inner-accordion-content");

        // active
        {
            let bloom_active_content_element =
                gloo::utils::document().create_element("div").unwrap();
            bloom_active_content_element.set_class_name("widget-value");

            {
                let bloom_active_input_checkbox =
                    create_checkbox_input("bloom-active", scene_value.parameters.is_use_bloom);

                {
                    let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                    let bloom_active_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                        wasm_bindgen::closure::Closure::wrap(Box::new(
                            move |_event: web_sys::InputEvent| {
                                let bloom_active_element: web_sys::Element =
                                    gloo::utils::document()
                                        .get_element_by_id("bloom-active")
                                        .unwrap();
                                let bloom_active_element: web_sys::HtmlInputElement =
                                    bloom_active_element.dyn_into().unwrap();
                                let value: bool = bloom_active_element.checked();

                                let mut scene_value = scene_clone.borrow_mut();
                                scene_value.parameters.is_use_bloom = value;
                            },
                        )
                            as Box<dyn FnMut(_)>);

                    bloom_active_input_checkbox
                        .add_event_listener_with_callback(
                            "input",
                            bloom_active_closure.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    bloom_active_closure.forget();
                }

                bloom_active_content_element
                    .append_child(&bloom_active_input_checkbox)
                    .unwrap();
            }

            let bloom_active_element = widget_row!("Active", &bloom_active_content_element);

            bloom_accordion_content_element
                .append_child(&bloom_active_element)
                .unwrap();
        }

        // threshold
        {
            let (threshold_element, threshold_content_element) =
                create_widget_row("Threshold");
            let threshold_input_range = create_range_input(
                "threshold-range",
                scene_value.parameters.bloom_threshold,
                "0.0",
                "2.0",
                "0.01",
            );
            let threshold_input_range_text = create_range_value_text(
                "threshold-range-text",
                scene_value.parameters.bloom_threshold,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                let threshold_range_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let threshold_element: web_sys::Element = gloo::utils::document()
                                .get_element_by_id("threshold-range")
                                .unwrap();
                            let threshold_element: web_sys::HtmlInputElement =
                                threshold_element.dyn_into().unwrap();
                            let value: String = threshold_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.bloom_threshold = value.parse::<f32>().unwrap();

                            let threshold_text_element: web_sys::Element =
                                gloo::utils::document()
                                    .get_element_by_id("threshold-range-text")
                                    .unwrap();
                            threshold_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                threshold_input_range
                    .add_event_listener_with_callback(
                        "input",
                        threshold_range_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                threshold_range_closure.forget();
            }

            threshold_content_element
                .append_child(&threshold_input_range)
                .unwrap();
            threshold_content_element
                .append_child(&threshold_input_range_text)
                .unwrap();

            bloom_accordion_content_element
                .append_child(&threshold_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&bloom_accordion_input_element)
            .unwrap();
        accordion_content_element
            .append_child(&bloom_accordion_label_element)
            .unwrap();
        accordion_content_element
            .append_child(&bloom_accordion_content_element)
            .unwrap();
    }

    // composite
    {
        let composite_accordion_input_element =
            gloo::utils::document().create_element("input").unwrap();
        let composite_accordion_input_element: web_sys::HtmlInputElement =
            composite_accordion_input_element.dyn_into().unwrap();
        composite_accordion_input_element
            .set_attribute("type", "checkbox")
            .unwrap();
        composite_accordion_input_element.set_class_name("accordion-input");
        composite_accordion_input_element.set_id("accordion-composite");
        //composite_accordion_input_element.set_checked(true);

        let composite_accordion_label_element =
            gloo::utils::document().create_element("label").unwrap();
        composite_accordion_label_element.set_class_name("accordion-label inner-accordion-label");
        composite_accordion_label_element.set_text_content(Some("Composite"));
        composite_accordion_label_element
            .set_attribute("for", "accordion-composite")
            .unwrap();

        let composite_accordion_content_element =
            gloo::utils::document().create_element("div").unwrap();
        composite_accordion_content_element
            .set_class_name("accordion-content inner-accordion-content");

        // active
        {
            let composite_active_content_element =
                gloo::utils::document().create_element("div").unwrap();
            composite_active_content_element.set_class_name("widget-value");

            {
                let composite_active_input_checkbox = create_checkbox_input(
                    "composite-active",
                    scene_value.parameters.is_use_composite,
                );

                {
                    let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                    let composite_active_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                        wasm_bindgen::closure::Closure::wrap(Box::new(
                            move |_event: web_sys::InputEvent| {
                                let composite_active_element: web_sys::Element =
                                    gloo::utils::document()
                                        .get_element_by_id("composite-active")
                                        .unwrap();
                                let composite_active_element: web_sys::HtmlInputElement =
                                    composite_active_element.dyn_into().unwrap();
                                let value: bool = composite_active_element.checked();

                                let mut scene_value = scene_clone.borrow_mut();
                                scene_value.parameters.is_use_composite = value;
                            },
                        )
                            as Box<dyn FnMut(_)>);

                    composite_active_input_checkbox
                        .add_event_listener_with_callback(
                            "input",
                            composite_active_closure.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    composite_active_closure.forget();
                }

                composite_active_content_element
                    .append_child(&composite_active_input_checkbox)
                    .unwrap();
            }

            let composite_active_element = widget_row!("Active", &composite_active_content_element);

            composite_accordion_content_element
                .append_child(&composite_active_element)
                .unwrap();
        }

        // exposure
        {
            let (composite_exposure_element, composite_exposure_content_element) =
                create_widget_row("Exposure");
            let composite_exposure_input_range = create_range_input(
                "composite-exposure-range",
                scene_value.parameters.composite_exposure,
                "-3.0",
                "3.0",
                "0.01",
            );
            let composite_exposure_input_range_text = create_range_value_text(
                "composite-exposure-range-text",
                scene_value.parameters.composite_exposure,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_exposure_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-exposure-range").unwrap();
                            let range_element: web_sys::HtmlInputElement =
                                range_element.dyn_into().unwrap();
                            let value: String = range_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_exposure = value.parse::<f32>().unwrap();

                            let range_text_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-exposure-range-text").unwrap();
                            range_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_exposure_input_range
                    .add_event_listener_with_callback(
                        "input",
                        composite_exposure_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_exposure_closure.forget();
            }

            composite_exposure_content_element
                .append_child(&composite_exposure_input_range)
                .unwrap();
            composite_exposure_content_element
                .append_child(&composite_exposure_input_range_text)
                .unwrap();

            composite_accordion_content_element
                .append_child(&composite_exposure_element)
                .unwrap();
        }

        // saturation
        {
            let (composite_saturation_element, composite_saturation_content_element) =
                create_widget_row("Saturation");
            let composite_saturation_input_range = create_range_input(
                "composite-saturation-range",
                scene_value.parameters.composite_saturation,
                "0.0",
                "2.0",
                "0.01",
            );
            let composite_saturation_input_range_text = create_range_value_text(
                "composite-saturation-range-text",
                scene_value.parameters.composite_saturation,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_saturation_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-saturation-range").unwrap();
                            let range_element: web_sys::HtmlInputElement =
                                range_element.dyn_into().unwrap();
                            let value: String = range_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_saturation = value.parse::<f32>().unwrap();

                            let range_text_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-saturation-range-text").unwrap();
                            range_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_saturation_input_range
                    .add_event_listener_with_callback(
                        "input",
                        composite_saturation_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_saturation_closure.forget();
            }

            composite_saturation_content_element
                .append_child(&composite_saturation_input_range)
                .unwrap();
            composite_saturation_content_element
                .append_child(&composite_saturation_input_range_text)
                .unwrap();

            composite_accordion_content_element
                .append_child(&composite_saturation_element)
                .unwrap();
        }

        // tone mapping
        {
            let composite_tone_mapping_content_element =
                gloo::utils::document().create_element("div").unwrap();
            composite_tone_mapping_content_element.set_class_name("widget-value");

            let composite_tone_mapping_select_element = create_select_input(
                "composite-tone-mapping-select",
                &[("off", "off"), ("aces", "aces"), ("filmic", "filmic"), ("agx", "agx")],
                Some(match scene_value.parameters.tone_mapping_type {
                    engine::scene::ToneMappingType::Off => "off",
                    engine::scene::ToneMappingType::Aces => "aces",
                    engine::scene::ToneMappingType::Filmic => "filmic",
                    engine::scene::ToneMappingType::Agx => "agx",
                }),
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_tone_mapping_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let select_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-tone-mapping-select").unwrap();
                            let select_element: web_sys::HtmlSelectElement =
                                select_element.dyn_into().unwrap();
                            let value: String = select_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.tone_mapping_type = match value.as_str() {
                                "off" => engine::scene::ToneMappingType::Off,
                                "aces" => engine::scene::ToneMappingType::Aces,
                                "filmic" => engine::scene::ToneMappingType::Filmic,
                                "agx" => engine::scene::ToneMappingType::Agx,
                                _ => engine::scene::ToneMappingType::Agx,
                            };
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_tone_mapping_select_element
                    .add_event_listener_with_callback(
                        "change",
                        composite_tone_mapping_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_tone_mapping_closure.forget();
            }

            composite_tone_mapping_content_element
                .append_child(&composite_tone_mapping_select_element)
                .unwrap();
            let composite_tone_mapping_element =
                widget_row!("Tone Mapping", &composite_tone_mapping_content_element);

            composite_accordion_content_element
                .append_child(&composite_tone_mapping_element)
                .unwrap();
        }

        // highlight rolloff
        {
            let (composite_highlight_rolloff_element, composite_highlight_rolloff_content_element) =
                create_widget_row("Soft");
            let composite_highlight_rolloff_input_range = create_range_input(
                "composite-highlight-rolloff-range",
                scene_value.parameters.composite_highlight_rolloff,
                "0.0",
                "1.0",
                "0.01",
            );
            let composite_highlight_rolloff_input_range_text = create_range_value_text(
                "composite-highlight-rolloff-range-text",
                scene_value.parameters.composite_highlight_rolloff,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_highlight_rolloff_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-highlight-rolloff-range").unwrap();
                            let range_element: web_sys::HtmlInputElement =
                                range_element.dyn_into().unwrap();
                            let value: String = range_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_highlight_rolloff = value.parse::<f32>().unwrap();

                            let range_text_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-highlight-rolloff-range-text").unwrap();
                            range_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_highlight_rolloff_input_range
                    .add_event_listener_with_callback(
                        "input",
                        composite_highlight_rolloff_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_highlight_rolloff_closure.forget();
            }

            composite_highlight_rolloff_content_element
                .append_child(&composite_highlight_rolloff_input_range)
                .unwrap();
            composite_highlight_rolloff_content_element
                .append_child(&composite_highlight_rolloff_input_range_text)
                .unwrap();

            composite_accordion_content_element
                .append_child(&composite_highlight_rolloff_element)
                .unwrap();
        }

        // white point
        {
            let (composite_white_point_element, composite_white_point_content_element) =
                create_widget_row("White");
            let composite_white_point_input_range = create_range_input(
                "composite-white-point-range",
                scene_value.parameters.composite_white_point,
                "0.1",
                "4.0",
                "0.01",
            );
            let composite_white_point_input_range_text = create_range_value_text(
                "composite-white-point-range-text",
                scene_value.parameters.composite_white_point,
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_white_point_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let range_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-white-point-range").unwrap();
                            let range_element: web_sys::HtmlInputElement =
                                range_element.dyn_into().unwrap();
                            let value: String = range_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_white_point = value.parse::<f32>().unwrap();

                            let range_text_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-white-point-range-text").unwrap();
                            range_text_element.set_text_content(Some(&value));
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_white_point_input_range
                    .add_event_listener_with_callback(
                        "input",
                        composite_white_point_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_white_point_closure.forget();
            }

            composite_white_point_content_element
                .append_child(&composite_white_point_input_range)
                .unwrap();
            composite_white_point_content_element
                .append_child(&composite_white_point_input_range_text)
                .unwrap();

            composite_accordion_content_element
                .append_child(&composite_white_point_element)
                .unwrap();
        }

        // view transform
        {
            let composite_view_transform_content_element =
                gloo::utils::document().create_element("div").unwrap();
            composite_view_transform_content_element.set_class_name("widget-value");

            let composite_view_transform_select_element = create_select_input(
                "composite-view-transform-select",
                &[("standard", "standard"), ("agx", "agx"), ("filmic", "filmic")],
                Some(match scene_value.parameters.composite_view_transform {
                    engine::scene::CompositeViewTransform::Standard => "standard",
                    engine::scene::CompositeViewTransform::Agx => "agx",
                    engine::scene::CompositeViewTransform::Filmic => "filmic",
                }),
            );

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_view_transform_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let select_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-view-transform-select").unwrap();
                            let select_element: web_sys::HtmlSelectElement =
                                select_element.dyn_into().unwrap();
                            let value: String = select_element.value();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_view_transform = match value.as_str() {
                                "standard" => engine::scene::CompositeViewTransform::Standard,
                                "agx" => engine::scene::CompositeViewTransform::Agx,
                                "filmic" => engine::scene::CompositeViewTransform::Filmic,
                                _ => engine::scene::CompositeViewTransform::Agx,
                            };
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_view_transform_select_element
                    .add_event_listener_with_callback(
                        "change",
                        composite_view_transform_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_view_transform_closure.forget();
            }

            composite_view_transform_content_element
                .append_child(&composite_view_transform_select_element)
                .unwrap();
            let composite_view_transform_element =
                widget_row!("View Trans", &composite_view_transform_content_element);

            composite_accordion_content_element
                .append_child(&composite_view_transform_element)
                .unwrap();
        }

        // dither
        {
            let composite_dither_content_element =
                gloo::utils::document().create_element("div").unwrap();
            composite_dither_content_element.set_class_name("widget-value");

            let composite_dither_input_checkbox =
                create_checkbox_input("composite-dither-active", scene_value.parameters.composite_use_dither);

            {
                let scene_clone: Shared<engine::scene::Scene> = scene.clone();
                let composite_dither_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                    wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_event: web_sys::InputEvent| {
                            let composite_dither_element: web_sys::Element =
                                gloo::utils::document().get_element_by_id("composite-dither-active").unwrap();
                            let composite_dither_element: web_sys::HtmlInputElement =
                                composite_dither_element.dyn_into().unwrap();
                            let value: bool = composite_dither_element.checked();

                            let mut scene_value = scene_clone.borrow_mut();
                            scene_value.parameters.composite_use_dither = value;
                        },
                    ) as Box<dyn FnMut(_)>);

                composite_dither_input_checkbox
                    .add_event_listener_with_callback(
                        "input",
                        composite_dither_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                composite_dither_closure.forget();
            }

            composite_dither_content_element
                .append_child(&composite_dither_input_checkbox)
                .unwrap();
            let composite_dither_element = widget_row!("Dithered", &composite_dither_content_element);

            composite_accordion_content_element
                .append_child(&composite_dither_element)
                .unwrap();
        }

        // gamma correction
        {
            let gamma_correction_content_element =
                gloo::utils::document().create_element("div").unwrap();
            gamma_correction_content_element.set_class_name("widget-value");

            {
                let gamma_correction_input_checkbox = create_checkbox_input(
                    "gamma-correction-active",
                    scene_value.parameters.is_use_gamma_correction,
                );

                {
                    let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                    let gamma_correction_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                        wasm_bindgen::closure::Closure::wrap(Box::new(
                            move |_event: web_sys::InputEvent| {
                                let gamma_correction_element: web_sys::Element =
                                    gloo::utils::document()
                                        .get_element_by_id("gamma-correction-active")
                                        .unwrap();
                                let gamma_correction_element: web_sys::HtmlInputElement =
                                    gamma_correction_element.dyn_into().unwrap();
                                let value: bool = gamma_correction_element.checked();

                                let mut scene_value = scene_clone.borrow_mut();
                                scene_value.parameters.is_use_gamma_correction = value;
                            },
                        )
                            as Box<dyn FnMut(_)>);

                    gamma_correction_input_checkbox
                        .add_event_listener_with_callback(
                            "input",
                            gamma_correction_closure.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    gamma_correction_closure.forget();
                }

                gamma_correction_content_element
                    .append_child(&gamma_correction_input_checkbox)
                    .unwrap();
            }

            let gamma_correction_element = widget_row!("Gamma", &gamma_correction_content_element);

            composite_accordion_content_element
                .append_child(&gamma_correction_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&composite_accordion_input_element)
            .unwrap();
        accordion_content_element
            .append_child(&composite_accordion_label_element)
            .unwrap();
        accordion_content_element
            .append_child(&composite_accordion_content_element)
            .unwrap();
    }

    view_statistics
        .append_child(&accordion_input_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_label_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&view_statistics).unwrap();
}

fn create_debug_dialog_overlay(parent: &web_sys::Element, scene: &Shared<engine::scene::Scene>) {
    let scene_value = scene.borrow();

    let (dialog_overlay, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-overlay",
            "accordion-overlay",
            "Overlay",
            "accordion-label",
            "accordion-content",
        );

    // grid
    {
        let grid_accordion_input_element = gloo::utils::document().create_element("input").unwrap();
        let grid_accordion_input_element: web_sys::HtmlInputElement =
            grid_accordion_input_element.dyn_into().unwrap();
        grid_accordion_input_element
            .set_attribute("type", "checkbox")
            .unwrap();
        grid_accordion_input_element.set_class_name("accordion-input");
        grid_accordion_input_element.set_id("accordion-grid");
        //grid_accordion_input_element.set_checked(true);

        let grid_accordion_label_element = gloo::utils::document().create_element("label").unwrap();
        grid_accordion_label_element.set_class_name("accordion-label inner-accordion-label");
        grid_accordion_label_element.set_text_content(Some("Grid"));
        grid_accordion_label_element
            .set_attribute("for", "accordion-grid")
            .unwrap();

        let grid_accordion_content_element = gloo::utils::document().create_element("div").unwrap();
        grid_accordion_content_element.set_class_name("accordion-content inner-accordion-content");

        // active
        {
            let grid_active_content_element =
                gloo::utils::document().create_element("div").unwrap();
            grid_active_content_element.set_class_name("widget-value");

            {
                let grid_active_input_checkbox =
                    create_checkbox_input("grid-active", scene_value.parameters.is_use_grid);

                {
                    let scene_clone: Shared<engine::scene::Scene> = scene.clone();

                    let grid_active_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
                        wasm_bindgen::closure::Closure::wrap(Box::new(
                            move |_event: web_sys::InputEvent| {
                                let grid_active_element: web_sys::Element = gloo::utils::document()
                                    .get_element_by_id("grid-active")
                                    .unwrap();
                                let grid_active_element: web_sys::HtmlInputElement =
                                    grid_active_element.dyn_into().unwrap();
                                let value: bool = grid_active_element.checked();

                                let mut scene_value = scene_clone.borrow_mut();
                                scene_value.parameters.is_use_grid = value;
                            },
                        )
                            as Box<dyn FnMut(_)>);

                    grid_active_input_checkbox
                        .add_event_listener_with_callback(
                            "input",
                            grid_active_closure.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    grid_active_closure.forget();
                }

                grid_active_content_element
                    .append_child(&grid_active_input_checkbox)
                    .unwrap();
            }

            let grid_active_element = widget_row!("Active", &grid_active_content_element);

            grid_accordion_content_element
                .append_child(&grid_active_element)
                .unwrap();
        }

        accordion_content_element
            .append_child(&grid_accordion_input_element)
            .unwrap();
        accordion_content_element
            .append_child(&grid_accordion_label_element)
            .unwrap();
        accordion_content_element
            .append_child(&grid_accordion_content_element)
            .unwrap();
    }

    dialog_overlay
        .append_child(&accordion_input_element)
        .unwrap();
    dialog_overlay
        .append_child(&accordion_label_element)
        .unwrap();
    dialog_overlay
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&dialog_overlay).unwrap();
}

fn create_debug_dialog_statistics(parent: &web_sys::Element, scene: &Shared<engine::scene::Scene>) {

    let (view_statistics, accordion_input_element, accordion_label_element, accordion_content_element) =
        create_accordion_section(
            "dialog-element-analytics",
            "accordion-analytics",
            "Statistics",
            "accordion-label",
            "accordion-content",
        );

    // objects
    {
        let objects_stats_content_element = gloo::utils::document().create_element("div").unwrap();
        objects_stats_content_element.set_class_name("widget-value");
        objects_stats_content_element.set_id("objects-analytics-value");
        objects_stats_content_element
            .set_text_content(Some(scene.borrow().objects.len().to_string().as_str()));

        let objects_element = widget_row!("Objects", &objects_stats_content_element);

        accordion_content_element
            .append_child(&objects_element)
            .unwrap();
    }

    // materials
    {
        let objects_stats_content_element = gloo::utils::document().create_element("div").unwrap();
        objects_stats_content_element.set_class_name("widget-value");
        objects_stats_content_element.set_id("materials-analytics-value");
        objects_stats_content_element
            .set_text_content(Some(scene.borrow().materials.len().to_string().as_str()));

        let materials_element = widget_row!("Materials", &objects_stats_content_element);

        accordion_content_element
            .append_child(&materials_element)
            .unwrap();
    }

    view_statistics
        .append_child(&accordion_input_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_label_element)
        .unwrap();
    view_statistics
        .append_child(&accordion_content_element)
        .unwrap();

    parent.append_child(&view_statistics).unwrap();
}
