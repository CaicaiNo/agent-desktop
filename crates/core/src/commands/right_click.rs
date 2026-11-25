use crate::{
    AppError,
    actions::action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
};
use serde_json::Value;

/// Delivers a right click and records the menu evidence it invalidates.
///
/// A right click is the one delivery that opens a context menu, so it is also
/// the one place the driver can say which element a later menu belongs to. The
/// menu read behind that evidence happens before the delivery, and neither its
/// result nor the record it feeds can turn a delivered click into a failure.
pub fn execute(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let pending = crate::commands::helpers::load_ref_entry(&args.ref_id, context)
        .ok()
        .and_then(|entry| crate::menu_origin::prepare(adapter, &args.ref_id, &entry));
    let request = context.request_base(Action::RightClick);
    let result = execute_ref_action_with_context(args, adapter, request, context);
    crate::menu_origin::finish(pending, result.is_ok(), context);
    result
}

#[cfg(test)]
#[path = "right_click_tests.rs"]
mod tests;
