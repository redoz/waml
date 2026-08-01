pub mod document;
pub mod edit;
pub mod history;
pub mod ime;
pub mod input;
pub mod layout;
pub mod motion;
pub mod presentation;
pub mod selection;
pub mod session;
pub mod unicode;
pub mod widget;

pub fn script_mod(vm: &mut makepad_widgets::ScriptVm) -> makepad_widgets::ScriptValue {
    widget::register_script_mod(vm)
}

pub fn live_design(cx: &mut makepad_widgets::Cx) {
    cx.with_vm(makepad_widgets::script_mod);
    cx.with_vm(script_mod);
}
