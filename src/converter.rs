use spirv_cross::{
    glsl,
    spirv,
};

use shaderc;

use std::{
    iter,
    path::{Path, PathBuf},
    fs::File,
    io::Read,
    collections::HashMap,
};

use GlslVersion;
use Stage;
use ConvertedShader;
use error::Error;

#[derive(Debug, Clone)]
pub struct ConverterOptions {
    /// Additional directories to search in when resolving `#include` statements.
    ///
    /// The path to the file being converted is always implicity used as a search path, taking
    /// priority over any paths listed here.
    ///
    /// Next, the paths listed here are tried in order.
    pub include_search_paths: Vec<PathBuf>,

    /// Macros to `#define` during compilation. Use `None` to define the macro without a value.
    pub macros: HashMap<String, Option<String>>,

    pub target_version: GlslVersion,
}

impl Default for ConverterOptions {
    fn default() -> Self {
        ConverterOptions {
            include_search_paths: Vec::new(),
            macros: HashMap::new(),

            target_version: GlslVersion::V1_00Es,
        }
    }
}

impl ConverterOptions {
    pub fn new() -> Self {
        Self::default()
    }

    fn resolve_include(&self,
                       name: &str,
                       include_type: shaderc::IncludeType,
                       _from_path: &str,
                       _depth: usize) -> Result<shaderc::ResolvedInclude, String> {
        let path = match (include_type, PathBuf::from(name).parent()) {
            (shaderc::IncludeType::Relative, Some(parent_path)) => {
                let mut search_paths_and_parent: Vec<_> = iter::once(parent_path)
                    .chain(self.include_search_paths.iter().map(|path_buf_ref| {
                        path_buf_ref as &Path
                    }))
                    .collect();

                find_source_file(name, &search_paths_and_parent)?
            }

            _ => find_source_file(name, &self.include_search_paths)?
        };

        let mut content = String::new();
        File::open(&path)
            .and_then(|mut include_file| include_file.read_to_string(&mut content))
            .map_err(|err| err.to_string())?;

        Ok(shaderc::ResolvedInclude {
            resolved_name: path.to_string_lossy().to_string(),
            content,
        })
    }
}

pub struct Converter {
    compiler: shaderc::Compiler,
}

impl Converter {
    pub fn new() -> Result<Self, Error> {
        let compiler = shaderc::Compiler::new()
            .ok_or(Error::InitFailed)?;

        Ok(Self {
            compiler
        })
    }

    /// Convert a HLSL file to GLSL.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Location of HLSL source file.
    /// * `stage` - Type of GLSL shader to create.
    /// * `entry_point` - Name of function to use as entry point for this stage in the HLSL source.
    /// * `options` - Converter configuration.
    pub fn convert<P>(
        &mut self,
        source_path: P,
        stage: Stage,
        entry_point: &str,
        options: &ConverterOptions) -> Result<ConvertedShader, Error>
        where P: Into<PathBuf>
    {
        let source_path = source_path.into();
        let source_filename = source_path.to_string_lossy();

        let mut source = String::new();
        File::open(&source_path)?.read_to_string(&mut source)?;

        let spirv = self.hlsl_to_spirv(&source,
                                       source_filename.as_ref(),
                                       stage,
                                       entry_point,
                                       options)?;
        let module = spirv::Module::from_words(&spirv);

        let mut ast = spirv::Ast::<glsl::Target>::parse(&module)?;
        spirv::Compile::set_compiler_options(&mut ast, &glsl::CompilerOptions {
            version: options.target_version,
            vertex: glsl::CompilerVertexOptions {
                invert_y: false,
                transform_clip_space: false,
            },
        })?;

        let shader = ast.compile()?;
        let uniforms = find_uniform_mappings(&ast)?;

        Ok(ConvertedShader {
            shader,
            uniforms,
        })
    }

    fn hlsl_to_spirv(&mut self,
                     source: &str,
                     source_filename: &str,
                     stage: Stage,
                     entry_point: &str,
                     options: &ConverterOptions) -> Result<Vec<u32>, Error> {
        let mut opts = shaderc::CompileOptions::new().ok_or(Error::InitFailed)?;
        opts.set_source_language(shaderc::SourceLanguage::HLSL);
        opts.set_target_env(shaderc::TargetEnv::Vulkan, 0);
        opts.set_optimization_level(shaderc::OptimizationLevel::Performance);
        opts.set_generate_debug_info();
        opts.set_include_callback(|name, include_type, from_path, depth| {
            options.resolve_include(name, include_type, from_path, depth)
        });

        for (macro_name, macro_value) in options.macros.iter() {
            opts.add_macro_definition(macro_name, macro_value.as_ref().map(|val| val.as_str()));
        }

        let kind = match stage {
            Stage::Fragment => shaderc::ShaderKind::Fragment,
            Stage::Vertex => shaderc::ShaderKind::Vertex,
        };

        let artifact = self.compiler.compile_into_spirv(
            &source,
            kind,
            source_filename,
            entry_point,
            Some(&opts))?;

        if artifact.get_num_warnings() > 0 {
            warn!("{}", artifact.get_warning_messages());
        }

        Ok(artifact.as_binary().to_vec())
    }
}

fn find_uniform_mappings(ast: &spirv::Ast<glsl::Target>)
                         -> Result<HashMap<String, String>, Error> {
    let shader_resources = ast.get_shader_resources()?;

    let mut mappings = HashMap::new();

    /* discover property indices from debug names in the uniform buffers */
    for uniform_buffer in shader_resources.uniform_buffers {
        for member_name in get_member_names_deep(&ast, uniform_buffer.base_type_id)? {
            let flat_name = format!("_{}.{}", uniform_buffer.id, member_name);

            mappings.insert(flat_name, member_name);
        }
    }

    /* samplers end up in sampled_images, separate_images and separate_samplers - final IDs
     are from sampled_images (the combined sampler resource), and names are from separate_images
     (the Texture2D) */
    for (image_index, sampled_image) in shader_resources.sampled_images.into_iter().enumerate() {
        let image = &shader_resources.separate_images[image_index];

        let compiled_name = format!("_{}", sampled_image.id);

        mappings.insert(compiled_name, image.name.to_string());
    }

    Ok(mappings)
}

fn get_member_names_deep(ast: &spirv::Ast<glsl::Target>,
                         struct_type_id: u32)
                         -> Result<Vec<String>, Error> {
    let (member_types, _member_array_sizes) = match ast.get_type(struct_type_id)? {
        spirv::Type::Struct { member_types, array } => (member_types, array),
        _ => panic!("uniform buffer must be a struct"),
    };

    let mut names = Vec::new();
    for (member_id, member_type) in member_types.into_iter().enumerate() {
        let member_id = member_id as u32;

        let member_base_name = ast.get_member_name(struct_type_id, member_id)?;

        match ast.get_type(member_type)? {
            spirv::Type::Struct { ref array, .. } => {
                let element_names = array_member_names(&member_base_name, array);

                let member_base_type = ast.get_base_type_id(member_type)?;
                let child_names = get_member_names_deep(ast, member_base_type)?;

                for element_name in element_names {
                    for child_name in child_names.iter() {
                        names.push(format!("{}.{}", element_name, child_name.clone()));
                    }
                }
            }

            spirv::Type::Float { ref array } |
            spirv::Type::Double { ref array } |
            spirv::Type::Int { ref array } |
            spirv::Type::Int64 { ref array } |
            spirv::Type::UInt { ref array } |
            spirv::Type::UInt64 { ref array } |
            spirv::Type::Boolean { ref array } |
            spirv::Type::Char { ref array } |
            spirv::Type::Half { ref array } => {
                names.extend(array_member_names(&member_base_name, array));
            }

            spirv::Type::Image { .. } |
            spirv::Type::SampledImage { .. } |
            spirv::Type::Sampler { .. } |
            spirv::Type::AtomicCounter { .. } |
            spirv::Type::Void |
            spirv::Type::Unknown => {
                let msg = format!("member of {} had an unsupported type", member_base_name);
                return Err(Error::CompilationFailed(msg));
            }
        }
    }

    Ok(names)
}

fn array_member_names(base_name: &str, array_dims: &[u32]) -> Vec<String> {
    if array_dims.len() == 0 {
        return vec![base_name.to_string()];
    }

    let mut array_element_names = Vec::new();

    for (rank, dim) in array_dims.iter().enumerate() {
        let prev_elements = array_element_names.clone();
        array_element_names.clear();

        for element in 0..*dim {
            if rank == 0 {
                array_element_names.push(format!("{}[{}]", base_name, element));
            } else {
                for prev_element in prev_elements.iter() {
                    array_element_names.push(format!("{}[{}]", prev_element, element));
                }
            }
        }
    }

    array_element_names
}

fn find_source_file<P>(name: &str, source_paths: &[P]) -> Result<PathBuf, String>
    where P: AsRef<Path>
{
    source_paths.iter()
        .filter_map(|path| {
            let file_path = path.as_ref().join(name);

            if file_path.exists() {
                Some(file_path)
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| format!(
            "unable to find shader file `{}` in search paths:\n{}",
            name,
            source_paths.iter()
                .map(|path| format!(" * `{}`", path.as_ref().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n"),
        ))
}
