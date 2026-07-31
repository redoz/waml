pub mod document;
pub mod edit;
pub mod history;
pub mod ime;
pub mod input;
pub mod layout;
pub mod selection;
pub mod session;
pub mod unicode;
pub mod widget;

pub fn live_design(cx: &mut makepad_widgets::Cx) {
    cx.with_vm(makepad_widgets::script_mod);
    widget::live_design(cx);
}
