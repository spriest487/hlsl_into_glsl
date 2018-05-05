extern crate shaderc;
extern crate spirv_cross;

#[macro_use]
extern crate log;

pub mod error;
pub mod converter;

#[cfg(test)]
extern crate tempfile;

#[cfg(test)]
extern crate regex;

#[cfg(test)]
mod test;

pub use self::{
    error::Error,
    converter::{ Converter, ConverterOptions, }
};

pub use self::spirv_cross::glsl::Version as GlslVersion;

use std::{
    collections::HashMap,
};

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Stage {
    Fragment,
    Vertex,
}

#[derive(Clone, Debug)]
pub struct ConvertedShader {
    shader: String,
    uniforms: HashMap<String, String>,
}

