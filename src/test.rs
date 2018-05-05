use std::{
    io::Write,
    fs::{self, File},
};
use tempfile;
use regex::*;
use super::*;

fn compile_test_shader(stage: Stage, entry_point: &str, src: &str) -> ConvertedShader {
    let tmp_dir = tempfile::tempdir()
        .expect("faild to create temp dir for shader")
        .into_path();

    let result = {
        let tmp_path = tmp_dir.join("test.hlsl");
        let mut tmp_file = File::create(&tmp_path)
            .expect("failed to create temp file for shader");

        tmp_file.write_all(src.as_bytes())
            .expect("failed to write temp shader file");

        let mut converter = Converter::new()
            .expect("converter init failed");

        let opts = ConverterOptions {
            target_version: GlslVersion::V1_50,
            ..ConverterOptions::default()
        };

        converter.convert(&tmp_path, stage, entry_point, &opts)
            .expect("compilation failed")
    };

    fs::read_dir(tmp_dir)
        .expect("failed to clean up temp dir");

    result
}

fn get_ubo_member_mappings(shader: &ConvertedShader) -> HashMap<String, String> {
    // there will be a prefix for the UBO struct, property name lookup doesn't care about
    // that so neither do we
    // the form is `_something.(the name)`
    let field_pattern = Regex::new(r"^\w+\.(.+)$").unwrap();

    shader.uniforms.iter()
        .map(|(compiled_name, mapped_name)| {
            let field_captures = field_pattern.captures(compiled_name)
                .expect("ubo field mattern must patch");

            let ubo_member_name = field_captures[1].to_string();

            (ubo_member_name, mapped_name.to_string())
        })
        .collect()
}


fn assert_member_matches(members: &HashMap<String, String>, member: &str) {
    assert_eq!(Some(&member.to_string()), members.get(member));
}


#[test]
fn ubo_struct_field_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        struct A { float b; } a;

        float4 vertex(): SV_POSITION { return float4(0.0, 0.0, 0.0, a.b); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(1, ubo_members.len());
    assert_member_matches(&ubo_members, "a.b");
}

#[test]
fn ubo_nested_struct_field_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        struct A {
            struct B { float c; } b;
        } a;

        float4 vertex(): SV_POSITION { return float4(0.0, 0.0, 0.0, a.b.c); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(1, ubo_members.len());
    assert_member_matches(&ubo_members, "a.b.c");
}

#[test]
fn ubo_array_field_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        float a[3];

        float4 vertex(): SV_POSITION { return float4(a[0], a[1], a[2], 0.0); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(3, ubo_members.len());
    assert_member_matches(&ubo_members, "a[0]");
    assert_member_matches(&ubo_members, "a[1]");
    assert_member_matches(&ubo_members, "a[2]");
}

#[test]
fn ubo_multidimensional_array_field_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        float a[2][2][2];

        float4 vertex(): SV_POSITION { return float4(a[0][0][0], a[0][1][0], a[1][1][1], 0.0); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(8, ubo_members.len());
    assert_member_matches(&ubo_members, "a[0][0][0]");
    assert_member_matches(&ubo_members, "a[0][0][1]");
    assert_member_matches(&ubo_members, "a[0][1][1]");
    assert_member_matches(&ubo_members, "a[0][1][0]");
    assert_member_matches(&ubo_members, "a[1][0][0]");
    assert_member_matches(&ubo_members, "a[1][0][1]");
    assert_member_matches(&ubo_members, "a[1][1][1]");
    assert_member_matches(&ubo_members, "a[1][1][0]");
}

#[test]
fn ubo_array_field_in_struct_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        struct A {
            float b[3];
        } a;

        float4 vertex(): SV_POSITION { return float4(a.b[0], a.b[1], a.b[2], 0.0); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(3, ubo_members.len());
    assert_member_matches(&ubo_members, "a.b[0]");
    assert_member_matches(&ubo_members, "a.b[1]");
    assert_member_matches(&ubo_members, "a.b[2]");
}

#[test]
fn ubo_array_of_struct_has_prop_mapping() {
    let shader = compile_test_shader(Stage::Vertex, "vertex", r"
        struct A { float b; };
        A a[2];

        float4 vertex(): SV_POSITION { return float4(a[0].b, a[1].b, 0.0, 0.0); }
    ");

    let ubo_members = get_ubo_member_mappings(&shader);

    assert_eq!(2, ubo_members.len());
    assert_member_matches(&ubo_members, "a[0].b");
    assert_member_matches(&ubo_members, "a[1].b");
}
