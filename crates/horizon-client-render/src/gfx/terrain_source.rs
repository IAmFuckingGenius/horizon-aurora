//! Legacy GLES lowering of the terrain shader (`gfx::terrain_shader`). Vulkan retains the
//! original WGSL byte for byte.

/// The terrain shader `source` as a device of the GL backend (`gl`) takes it.
pub(super) fn lowered(source: String, gl: bool) -> String {
    if gl { gles_source(&source) } else { source }
}

/// GLES 3.0 cannot declare arrays of arrays. Two adjacent vec4 entries
/// retain the original uniform byte layout and every colored-light value.
pub(super) fn gles_uniforms(source: &str) -> String {
    source
        .replace("array<array<vec4<f32>, 2>, 16>", "array<vec4<f32>, 32>")
        .replace(
            "globals.colored_lights[i][0]",
            "globals.colored_lights[2u * i]",
        )
        .replace(
            "globals.colored_lights[i][1]",
            "globals.colored_lights[2u * i + 1u]",
        )
}

fn gles_source(source: &str) -> String {
    let source = gles_uniforms(source).replace("\r\n", "\n");
    let source = unroll_emissive_lookup(source);
    let start = source
        .find("fn sprite_grad(")
        .expect("terrain sprite function");
    let level = source
        .find("fn sprite_level(")
        .expect("terrain level function");
    let end = level + source[level..].find("\n}\n").expect("terrain level end") + 3;
    let rgss = source
        .find("fn rotated_grid(")
        .expect("terrain RGSS function");
    let rgss_end = rgss + source[rgss..].find("\n}\n").expect("terrain RGSS end") + 3;
    let mut output = String::with_capacity(source.len());
    output.push_str(&source[..start]);
    output.push_str(include_str!("terrain_gles.wgsl.inc"));
    output.push_str(&source[end..rgss]);
    output.push_str(&source[rgss_end..]);

    // The stages already pass terrain's varyings packed in five vec4s (`TerrainWire`), the
    // integers flat: what Adreno 304's broken scalar varyings needed here before.
    output
}

// Adreno 710's compiler fails to link this dynamic uniform-array search in a
// robust EGL context. The pairs are sorted by base layer and the search stops
// at the first one at or past the sprite's: the branches do the same, each with
// a constant index, in groups of 32 so a short list skips the rest at once.
// Robust access stays on.
fn unroll_emissive_lookup(mut source: String) -> String {
    let start = source
        .find("    for (var i = 0u; i < min(globals.emissive_count.x, 128u); i++) {")
        .expect("terrain emissive lookup");
    let body = source[start..].find('{').unwrap() + start;
    let mut depth = 1;
    let mut end = body + 1;
    while depth != 0 {
        match source.as_bytes()[end] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        end += 1;
    }
    source.replace_range(start..end, &emissive_branches(0, 1));
    source
}
/// The search from pair `first` on, `depth` levels in; a group ends by handing the
/// next one on when the list is longer.
fn emissive_branches(first: u32, depth: usize) -> String {
    let pad = "    ".repeat(depth);
    let mut code = String::new();
    for i in first..first + 32 {
        let keyword = if i == first { "if" } else { "} else if" };
        code.push_str(&format!(
            "{pad}{keyword} min(globals.emissive_count.x, 128u) > {i}u && globals.emissive_layers[{i}].x >= sprite_layer {{\n\
             {pad}    if globals.emissive_layers[{i}].x == sprite_layer {{\n\
             {pad}        out.emissive_layer = globals.emissive_layers[{i}].y;\n\
             {pad}    }}\n"
        ));
    }
    if first + 32 < 128 {
        code.push_str(&format!(
            "{pad}}} else if min(globals.emissive_count.x, 128u) > {}u {{\n",
            first + 32
        ));
        code.push_str(&emissive_branches(first + 32, depth + 1));
    }
    code.push_str(&format!("{pad}}}\n"));
    code
}


#[cfg(test)]
#[path = "terrain_source/hdr_gpu_tests.rs"]
mod hdr_gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_source_is_unchanged_and_gles_validates_every_entrypoint() {
        let shader = super::super::terrain_shader();
        assert_eq!(lowered(shader.clone(), false), shader);
        let source = lowered(shader.clone(), true);
        let original = shader.replace("\r\n", "\n");
        assert_eq!(
            &source[source.find("struct SkyOut {").unwrap()..],
            &original[original.find("struct SkyOut {").unwrap()..],
            "sky and outline shader bodies must remain unchanged"
        );
        assert!(!source.contains("array<array<"));
        assert!(!source.contains("switch layer / SPRITE_SPLIT"));
        assert!(!source.contains("uv + offsets[i] * pixel"));
        for index in 0..7 {
            assert!(source.contains(&format!("array_index == {index}u")));
        }
        // The emissive search with constant indices only.
        assert!(!source.contains("globals.emissive_layers[i]"));
        let module = naga::front::wgsl::parse_str(&source).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let vertex = module
            .entry_points
            .iter()
            .find(|e| e.name == "vs_terrain")
            .unwrap();
        let naga::TypeInner::Struct { members, .. } =
            &module.types[vertex.function.result.as_ref().unwrap().ty].inner
        else {
            panic!("terrain result must be a struct")
        };
        assert_eq!(
            members
                .iter()
                .filter(|m| matches!(m.binding, Some(naga::Binding::Location { .. })))
                .count(),
            // `TerrainWire`'s five packed vec4s.
            5
        );
    }

    #[test]
    fn windows_line_endings_produce_the_same_gles_shader() {
        let source = super::super::terrain_shader().replace("\r\n", "\n");
        assert_eq!(
            gles_source(&source),
            gles_source(&source.replace('\n', "\r\n"))
        );
    }
}
