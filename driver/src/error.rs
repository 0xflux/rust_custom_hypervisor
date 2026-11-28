use alloc::string::String;

#[derive(Debug, thiserror::Error)]
pub enum HypervisorError {
    #[error("Memory allocation failed. Context: {0:?}")]
    AllocationFailed(Option<String>),
    #[error("VMX instruction failed: {0}")]
    VmxError(VmxError),
    #[error("A pointer which was expected to be non-null, was null. Context: {0:?}")]
    InvalidPtr(Option<String>),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("vmlaunch failed")]
    VmLaunchFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum VmxError {
    #[error("VMX instruction failed without recording an error code")]
    VmFailInvalid,
    #[error("VMX instruction failed and recorded an error code in the current VMCS")]
    VmFailValid,
}
