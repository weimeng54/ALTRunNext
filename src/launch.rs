//! Resolve an explicitly selected command without implicitly invoking a shell.
#[derive(Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub file: String,
    pub parameters: String,
}

pub fn resolve(_command: &str, _parameter: Option<&str>) -> Result<LaunchRequest, String> {
    Err("Command resolution is not implemented".into())
}
