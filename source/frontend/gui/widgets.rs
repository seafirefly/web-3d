use crate::engine::scene::Scene;
use crate::types::Shared;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

pub struct Slider {
    pub id: &'static str,
    pub label: &'static str,
    pub min: &'static str,
    pub max: &'static str,
    pub value: f32,
}

pub fn element(tag: &str) -> web_sys::Element {
    gloo::utils::document().create_element(tag).unwrap()
}

pub fn div(class_name: &str) -> web_sys::Element {
    let element = element("div");
    element.set_class_name(class_name);
    element
}

pub fn append(parent: &web_sys::Element, child: &web_sys::Element) {
    parent.append_child(child).unwrap();
}

fn listen(target: &web_sys::Element, event: &str, handler: impl FnMut() + 'static) {
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut()>);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();
}

fn input_value(element: &web_sys::Element) -> String {
    element.unchecked_ref::<web_sys::HtmlInputElement>().value()
}

fn input_checked(element: &web_sys::Element) -> bool {
    element
        .unchecked_ref::<web_sys::HtmlInputElement>()
        .checked()
}

pub fn widget_row(label: &str, children: &[&web_sys::Element]) -> web_sys::Element {
    let row = div("widget-row");

    let label_element = div("widget-label");
    label_element.set_text_content(Some(label));

    let value = div("widget-value");
    for child in children {
        append(&value, child);
    }

    append(&row, &label_element);
    append(&row, &value);
    row
}

pub fn value_wrapper(child: &web_sys::Element) -> web_sys::Element {
    let wrapper = div("widget-value");
    append(&wrapper, child);
    wrapper
}

pub fn stat_row(label: &str, id: &str, count: usize) -> web_sys::Element {
    let value = div("widget-value");
    value.set_id(id);
    value.set_text_content(Some(&count.to_string()));
    widget_row(label, &[&value])
}

pub fn slider_row(
    scene: &Shared<Scene>,
    slider: Slider,
    on_change: impl Fn(&mut Scene, f32) + 'static,
) -> web_sys::Element {
    let value_string = slider.value.to_string();

    let input = element("input");
    input.set_id(slider.id);
    input.set_class_name("range-element");
    input.set_attribute("type", "range").unwrap();
    input.set_attribute("min", slider.min).unwrap();
    input.set_attribute("max", slider.max).unwrap();
    input.set_attribute("step", "0.01").unwrap();
    input
        .unchecked_ref::<web_sys::HtmlInputElement>()
        .set_value(&value_string);

    let text = element("div");
    text.set_id(&format!("{}-text", slider.id));
    text.set_class_name("range-text-element");
    text.set_text_content(Some(&value_string));

    let scene = scene.clone();
    let input_target = input.clone();
    let text_target = text.clone();
    listen(&input, "input", move || {
        let value = input_value(&input_target);
        on_change(&mut scene.borrow_mut(), value.parse::<f32>().unwrap());
        text_target.set_text_content(Some(&value));
    });

    widget_row(slider.label, &[&input, &text])
}

pub fn checkbox(
    scene: &Shared<Scene>,
    id: &str,
    checked: bool,
    on_change: impl Fn(&mut Scene, bool) + 'static,
) -> web_sys::Element {
    let input = element("input");
    input.set_id(id);
    input.set_class_name("checkbox-element");
    input.set_attribute("type", "checkbox").unwrap();
    input
        .unchecked_ref::<web_sys::HtmlInputElement>()
        .set_checked(checked);

    let scene = scene.clone();
    let input_target = input.clone();
    listen(&input, "input", move || {
        on_change(&mut scene.borrow_mut(), input_checked(&input_target));
    });

    input
}

pub fn checkbox_row(
    scene: &Shared<Scene>,
    label: &str,
    id: &str,
    checked: bool,
    on_change: impl Fn(&mut Scene, bool) + 'static,
) -> web_sys::Element {
    let input = checkbox(scene, id, checked, on_change);
    widget_row(label, &[&value_wrapper(&input)])
}

pub fn select<T: Copy + PartialEq>(
    id: &str,
    options: &[(&'static str, T)],
    selected: Option<T>,
) -> web_sys::Element {
    let select = element("select");
    select.set_id(id);
    select.set_class_name("select-element");

    for (key, value) in options {
        let option = element("option");
        option.set_text_content(Some(key));
        option.set_attribute("value", key).unwrap();
        if selected == Some(*value) {
            option.set_attribute("selected", "").unwrap();
        }
        append(&select, &option);
    }

    select
}

pub fn on_select<T: Copy + 'static>(
    scene: &Shared<Scene>,
    select: &web_sys::Element,
    options: &[(&'static str, T)],
    on_change: impl Fn(&mut Scene, T) + 'static,
) {
    let options: Vec<(&'static str, T)> = options.to_vec();
    let scene = scene.clone();
    let select_target = select.clone();
    listen(select, "change", move || {
        let key = select_target
            .unchecked_ref::<web_sys::HtmlSelectElement>()
            .value();
        if let Some((_, value)) = options.iter().find(|(name, _)| *name == key) {
            on_change(&mut scene.borrow_mut(), *value);
        }
    });
}

pub fn color_picker(
    scene: &Shared<Scene>,
    id: &str,
    hex: &str,
    on_change: impl Fn(&mut Scene, &str) + 'static,
) -> web_sys::Element {
    let picker = element("input");
    picker.set_class_name("widget-value color-picker-element");
    picker.set_id(id);
    picker.set_attribute("type", "color").unwrap();
    picker.set_attribute("value", hex).unwrap();

    let scene = scene.clone();
    let picker_target = picker.clone();
    listen(&picker, "input", move || {
        let value = input_value(&picker_target);
        on_change(&mut scene.borrow_mut(), &value);
    });

    picker
}

pub fn accordion(
    parent: &web_sys::Element,
    name: &str,
    title: &str,
    nested: bool,
) -> web_sys::Element {
    let id = format!("accordion-{name}");

    let input = element("input");
    input.set_attribute("type", "checkbox").unwrap();
    input.set_class_name("accordion-input");
    input.set_id(&id);

    let label = element("label");
    label.set_class_name(if nested {
        "accordion-label inner-accordion-label"
    } else {
        "accordion-label"
    });
    label.set_text_content(Some(title));
    label.set_attribute("for", &id).unwrap();

    let content = div(if nested {
        "accordion-content inner-accordion-content"
    } else {
        "accordion-content"
    });

    append(parent, &input);
    append(parent, &label);
    append(parent, &content);
    content
}

pub fn dialog(parent: &web_sys::Element, name: &str, title: &str) -> web_sys::Element {
    let root = div("dialog-element dialog-element-display");
    root.set_id(&format!("dialog-element-{name}"));

    let content = accordion(&root, name, title, false);
    append(parent, &root);
    content
}
