use std::{
    io,
    fmt,
};

use shaderc;
use spirv_cross;

#[derive(Debug)]
pub enum Error {
    /// An error occured reading a source file.
    Io(io::Error),

    /// The native library could not be initialized.
    InitFailed,

    /// An error occured during GLSL generation.
    CompilationFailed(String),

    /// The HLSL source failed to parse.
    ParseFailed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Io(ref err) =>
                write!(f, "{}", err),
            &Error::InitFailed =>
                write!(f, "shader compiler initialization failed"),
            &Error::CompilationFailed(ref msg) |
            &Error::ParseFailed(ref msg) =>
                f.write_str(msg),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<spirv_cross::ErrorCode> for Error {
    fn from(err: spirv_cross::ErrorCode) -> Self {
        let msg = match err {
            spirv_cross::ErrorCode::Unhandled =>
                "unhandled".to_string(),
            spirv_cross::ErrorCode::CompilationError(msg) =>
                format!("compilation failed: {}", msg),
        };
        Error::ParseFailed(msg)
    }
}

impl From<shaderc::Error> for Error {
    fn from(err: shaderc::Error) -> Self {
        Error::CompilationFailed(err.to_string())
    }
}