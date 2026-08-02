use gpui::{App, Global};

#[derive(Default)]
struct MotionPolicy {
    reduced: bool,
}

impl Global for MotionPolicy {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(MotionPolicy::default());
}

/// Sets whether component animations should be reduced for this application.
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    cx.set_global(MotionPolicy { reduced });
}

pub(crate) fn is_reduced_motion(cx: &App) -> bool {
    cx.try_global::<MotionPolicy>()
        .is_some_and(|policy| policy.reduced)
}

#[cfg(test)]
mod tests {
    use super::MotionPolicy;

    #[test]
    fn motion_is_not_reduced_by_default() {
        assert!(!MotionPolicy::default().reduced);
    }
}
