/// Identifies the installed build used as the behavioral oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildReference {
    pub package_name: &'static str,
    pub package_version: &'static str,
    pub architecture: &'static str,
    pub runtime: &'static str,
}

pub const STABLE_REFERENCE: BuildReference = BuildReference {
    package_name: "OpenAI.Codex",
    package_version: "26.721.3996.0",
    architecture: "x64",
    runtime: "Owl/Chromium 150.0.7871.128",
};

#[must_use]
pub const fn stable_reference() -> &'static BuildReference {
    &STABLE_REFERENCE
}

#[cfg(test)]
mod tests {
    use super::stable_reference;

    #[test]
    fn stable_reference_is_pinned() {
        assert_eq!(stable_reference().package_version, "26.721.3996.0");
    }
}
