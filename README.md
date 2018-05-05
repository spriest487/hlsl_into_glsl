Write shaders in HLSL and compile them to GLSL and GLSL ES.

Powered by [shaderc](https://github.com/google/shaderc) and
[SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross).

The conversion process creates a GLSL fragment or vertex shader from a HLSL source file. It finds compiled uniform property names and maps them to the original property names, for easy use with `glUniform`.

## Example

```hlsl
float4 color;
        
struct VertexIn { 
    float3 pos: POSITION;
};

struct VertexOut { 
    float4 color: COLOR; 
};

float4 vertex_main(VertexIn vertex_in, out VertexOut vertex_out): SV_POSITION {
    vertex_out.color = color;
    return vertex_in.pos;
}

float4 fragment_main(VertOut vertex_out): SV_TARGET {
    return vertex_out.color;
}

```

```rust
extern crate hlsl_into_glsl;
use hlsl_into_glsl::{Converter, ConverterOptions, Stage};

fn main() {
    let opts = ConverterOptions::new();
    
    let mut converter = Converter::new()
        .unwrap();
        
    let vert = converter
        .convert("test.hlsl", Stage::Vertex, "vertex_main", &opts)
        .unwrap();
    let frag = converter
        .convert("test.hlsl", Stage::Fragment, "fragment_main", &opts)
        .unwrap();

    /* uniforms are returned as a map of compiled name -> friendly variable name */
    assert!(vert.uniforms.values().any(|var_name| var_name == "color"));
}
```

## Usage notes

### File paths

This tool is expected to be used as part of an offline asset compilation process, so it only supports loading shaders from disk. `#include` statements are resolved relative to the file being compiled. An additional list of search paths for includes can be provided in the `ConverterOptions` struct.

### Uniform mappings

Arrays and structs are supported. Resulting uniform name mappings are equivalent to those OpenGL would use for the same layout. For example, given the following HLSL uniform:

```hlsl
struct A { 
    float b; 
    float c[2]; 
} a[2];
```

The following properties will be generated:
* `a[0].b`
* `a[1].b`
* `a[0].c[0]`
* `a[1].c[0]`
* `a[0].c[1]`
* `a[1].c[1]`

### Stage inputs/outputs

For linking stages and binding vertex attributes with `glVertexAttribPointer` on versions of GLSL that don't support explicit layout, it's necessary to match the names of the attribute variables.

Names of vertex shader inputs are based on the name of the parameter and the member names. For example, an input paramter named `vert_in` of type `struct VertexIn { float3 pos: POSITION; }` results in the attribute name `vert_in_pos`.

To match the name of the vertex shader outputs with the fragment shader inputs, use an `out` parameter in the vertex shader which has the same name as the input parameter on the fragment shader.