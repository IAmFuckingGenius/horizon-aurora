//! Legacy GLES lowering. Vulkan retains the original WGSL byte for byte.
use std::borrow::Cow;

pub(super) fn terrain_shader(gl: bool) -> Cow<'static, str> {
    let source = include_str!("../terrain.wgsl");
    if gl {
        Cow::Owned(gles_source(source))
    } else {
        Cow::Borrowed(source)
    }
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

    // Adreno 304 renders broken terrain with the separated scalar varyings.
    // Pack values sharing their interpolation qualifier; leave the vertex
    // buffers, texture sampling, fade, lighting, fog and HDR arithmetic alone.
    let start = output
        .find("struct TerrainOut {")
        .expect("terrain interface");
    let fields_end = output[start..]
        .find("\n}\n")
        .expect("terrain interface end")
        + start
        + 3;
    // The terrain functions run up to the next stage interface (the native fluids',
    // then the sky's), whose own fields of the same names stay.
    let end = output[fields_end..]
        .match_indices("\nstruct ")
        .map(|(i, _)| fields_end + i + 1)
        .find(|&i| {
            output[i..]
                .split_once('{')
                .is_some_and(|(head, _)| head.trim_end().ends_with("Out"))
        })
        .expect("the next stage interface");
    let mut body = output[fields_end..end].to_owned();
    for (member, packed) in [
        ("emissive_layer", "codes.w"),
        ("layer", "codes.x"),
        ("emission", "codes.y"),
        ("fade", "codes.z"),
        ("spherical", "distances.x"),
        ("cylindrical", "distances.y"),
        ("visibility", "distances.z"),
        ("height", "distances.w"),
    ] {
        for variable in ["in", "out"] {
            body = body.replace(
                &format!("{variable}.{member}"),
                &format!("{variable}.{packed}"),
            );
        }
    }
    output.replace_range(start..end, &(PACKED_INTERFACE.to_owned() + &body));
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

const PACKED_INTERFACE: &str = "struct TerrainOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) codes: vec4<u32>,
    @location(2) color: vec4<f32>,
    @location(3) distances: vec4<f32>,
    @location(4) local: vec3<f32>,
    @location(5) scene: vec2<f32>,
}
";

#[cfg(test)]
#[path = "terrain_source/hdr_gpu_tests.rs"]
mod hdr_gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_source_is_unchanged_and_gles_validates_every_entrypoint() {
        assert!(matches!(terrain_shader(false), Cow::Borrowed(_)));
        assert_eq!(terrain_shader(false), include_str!("../terrain.wgsl"));
        let source = terrain_shader(true);
        let original = include_str!("../terrain.wgsl").replace("\r\n", "\n");
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
        assert!(source.contains("let whole = in.codes.y > 4u"));
        assert!(source.contains("if in.codes.w != 0xffffffffu"));
        assert!(!source.contains("in.height"));
        assert!(!source.contains("out.emissive_layer"));
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
            // uv, codes, color, distances, local, and HDR lighting's scene.
            6
        );
    }

    #[test]
    fn windows_line_endings_produce_the_same_gles_shader() {
        let source = include_str!("../terrain.wgsl").replace("\r\n", "\n");
        assert_eq!(
            gles_source(&source),
            gles_source(&source.replace('\n', "\r\n"))
        );
    }
}
