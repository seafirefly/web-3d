use crate::engine::constant;
use crate::types::Shared;

use wasm_bindgen::JsCast;

#[derive(Clone, Copy, Default)]
pub struct ControlResponseJs {
    pub movement_x: i32,
    pub movement_y: i32,
    pub on_left_click: bool,
    pub on_right_click: bool,
    pub wheel_delta_y: f64,
    pub on_wheel: bool,
    pub on_shift: bool,
    pub on_w: bool,
    pub on_a: bool,
    pub on_s: bool,
    pub on_d: bool,
}

pub fn add_event_listener_control(event_response: &Shared<ControlResponseJs>) {
    let canvas: web_sys::Element = gloo::utils::document()
        .get_element_by_id(constant::CANVAS_ELEMENT_ID)
        .unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().unwrap();

    let window: web_sys::Window = web_sys::window().unwrap();

    let response_clone_mouse: Shared<ControlResponseJs> = event_response.clone();

    let mouse_move_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let mut borrowed = response_clone_mouse.borrow_mut();

            borrowed.movement_x = event.movement_x();
            borrowed.movement_y = event.movement_y();
            borrowed.on_left_click = event.which() == 1;
            borrowed.on_right_click = event.which() == 3;

            /*
            borrowed.on_shift = event.get_modifier_state("Shift");
            borrowed.on_w = event.get_modifier_state("w");
            borrowed.on_a = event.get_modifier_state("a");
            borrowed.on_s = event.get_modifier_state("s");
            borrowed.on_d = event.get_modifier_state("d");
            */
        }) as Box<dyn FnMut(_)>);

    let response_clone_wheel: Shared<ControlResponseJs> = event_response.clone();

    let mouse_wheel_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
            let mut borrowed = response_clone_wheel.borrow_mut();

            borrowed.on_wheel = true;
            borrowed.wheel_delta_y = event.delta_y();
        }) as Box<dyn FnMut(_)>);

    let response_clone_key_down: Shared<ControlResponseJs> = event_response.clone();

    let key_down_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            let mut borrowed = response_clone_key_down.borrow_mut();

            if event.shift_key() {
                borrowed.on_shift = true;
            }
            if event.key() == "w" || event.key() == "W" {
                borrowed.on_w = true;
            }
            if event.key() == "a" || event.key() == "A" {
                borrowed.on_a = true;
            }
            if event.key() == "s" || event.key() == "S" {
                borrowed.on_s = true;
            }
            if event.key() == "d" || event.key() == "D" {
                borrowed.on_d = true;
            }
        }) as Box<dyn FnMut(_)>);

    let response_clone_key_up: Shared<ControlResponseJs> = event_response.clone();

    let key_up_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            let mut borrowed = response_clone_key_up.borrow_mut();

            if event.key() == "shift" {
                borrowed.on_shift = false;
            }
            if event.key() == "w" || event.key() == "W" {
                borrowed.on_w = false;
            }
            if event.key() == "a" || event.key() == "A" {
                borrowed.on_a = false;
            }
            if event.key() == "s" || event.key() == "S" {
                borrowed.on_s = false;
            }
            if event.key() == "d" || event.key() == "D" {
                borrowed.on_d = false;
            }
        }) as Box<dyn FnMut(_)>);

    let response_clone_blur: Shared<ControlResponseJs> = event_response.clone();

    let blur_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::UiEvent| {
            let mut borrowed = response_clone_blur.borrow_mut();
            borrowed.on_a = false;
            borrowed.on_d = false;
            borrowed.on_s = false;
            borrowed.on_w = false;
            borrowed.on_shift = false;
        }) as Box<dyn FnMut(_)>);

    let response_clone_context_menu: Shared<ControlResponseJs> = event_response.clone();

    let context_menu_closure: wasm_bindgen::prelude::Closure<dyn FnMut(_)> =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            let mut borrowed = response_clone_context_menu.borrow_mut();
            borrowed.on_a = false;
            borrowed.on_d = false;
            borrowed.on_s = false;
            borrowed.on_w = false;
            borrowed.on_shift = false;
        }) as Box<dyn FnMut(_)>);

    canvas
        .add_event_listener_with_callback("mousemove", mouse_move_closure.as_ref().unchecked_ref())
        .unwrap();
    mouse_move_closure.forget();

    canvas
        .add_event_listener_with_callback("wheel", mouse_wheel_closure.as_ref().unchecked_ref())
        .unwrap();
    mouse_wheel_closure.forget();

    window
        .add_event_listener_with_callback("keydown", key_down_closure.as_ref().unchecked_ref())
        .unwrap();
    key_down_closure.forget();

    window
        .add_event_listener_with_callback("keyup", key_up_closure.as_ref().unchecked_ref())
        .unwrap();
    key_up_closure.forget();

    window
        .add_event_listener_with_callback("blur", blur_closure.as_ref().unchecked_ref())
        .unwrap();
    blur_closure.forget();

    window
        .add_event_listener_with_callback(
            "contextmenu",
            context_menu_closure.as_ref().unchecked_ref(),
        )
        .unwrap();
    context_menu_closure.forget();
}
