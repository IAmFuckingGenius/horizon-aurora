#!/usr/bin/env python3
"""Check the renderer's WGSL on the host, as the game assembles it, with the naga it builds
with (third_party/naga): each shader parses and validates, and the GLES forms Aurora lowers
(terrain_source.rs, entity_source.rs) become GLSL ES for every entry point. No GPU, no game
data. A merge that leaves a shader that does not build fails here instead of at the
renderer's start on a phone.
"""
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RENDER = ROOT / 'crates/horizon-client-render/src'

# Parts the game only builds joined to others (the joined forms are checked below).
FRAGMENTS = {
    'entity.wgsl', 'static_entity.wgsl', 'oit_world.wgsl', 'oit_clouds.wgsl',
    'oit_features.wgsl', 'oit_border.wgsl', 'visbuffer.wgsl', 'gpu_occlusion.wgsl',
    'hdr_light.wgsl', 'custom_sky.wgsl', 'stars.wgsl', 'hdr.wgsl', 'world_warp_fragment.wgsl',
    'world_warp.wgsl',
}

MAIN = r'''
#[path = "@RENDER@/gfx/terrain_source.rs"]
mod terrain_source;
#[path = "@RENDER@/gfx/entity_source.rs"]
mod entity_source;
#[path = "@RENDER@/overlay.rs"]
#[allow(dead_code)]
mod overlay;
use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

/// Parses and validates `src`; with `gles`, writes every entry point as GLSL ES `gles`.
fn check(name: &str, src: &str, gles: Option<u16>) -> bool {
    let module = match naga::front::wgsl::parse_str(src) {
        Ok(m) => m,
        Err(e) => {
            println!("FAIL {name}: {}", e.emit_to_string(src));
            return false;
        }
    };
    let info = match Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module) {
        Ok(i) => i,
        Err(e) => {
            println!("FAIL {name}: {:?}", e.into_inner());
            return false;
        }
    };
    let mut ok = true;
    if let Some(version) = gles {
        for ep in &module.entry_points {
            let r = naga::back::pipeline_constants::process_overrides(
                &module,
                &info,
                Some((ep.stage, &ep.name)),
                &Default::default(),
            );
            let (m, i) = match r {
                Ok(x) => x,
                Err(e) => {
                    println!("FAIL {name}/{}: {e:?}", ep.name);
                    ok = false;
                    continue;
                }
            };
            let options = glsl::Options {
                version: glsl::Version::Embedded { version, is_webgl: false },
                ..Default::default()
            };
            let pipeline = glsl::PipelineOptions {
                shader_stage: ep.stage,
                entry_point: ep.name.clone(),
                multiview: None,
            };
            let mut out = String::new();
            let written = glsl::Writer::new(
                &mut out,
                &m,
                &i,
                &options,
                &pipeline,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .and_then(|mut w| w.write().map(|_| ()));
            if let Err(e) = written {
                println!("FAIL {name}/{}: GLSL ES {version}: {e:?}", ep.name);
                ok = false;
            }
        }
    }
    if ok {
        println!("ok   {name} ({} entry points)", module.entry_points.len());
    }
    ok
}

fn main() {
    let read = |file: &str| std::fs::read_to_string(format!("@RENDER@/{file}")).unwrap();
    let mut ok = true;
    // gfx.rs's terrain_shader and entity_shader.
    let terrain = read("hdr_light.wgsl") + &read("terrain.wgsl");
    ok &= check("terrain", &terrain, None);
    // The storage-buffer entry points need GLES 3.1, where the renderer makes them.
    let whole = terrain.clone();
    match std::panic::catch_unwind(move || terrain_source::lowered(whole, true)) {
        Ok(gles) => ok &= check("terrain (GLES)", &gles, Some(310)),
        Err(_) => {
            println!("FAIL terrain (GLES): the lowering panicked");
            ok = false;
        }
    }
    let entity = overlay::wgsl_constants() + &read("hdr_light.wgsl") + &read("entity.wgsl");
    ok &= check("entity", &entity, None);
    let gles_entity = terrain_source::gles_uniforms(&entity_source::legacy_overlay(&entity));
    ok &= check("entity (GLES)", &gles_entity, Some(300));
    ok &= check("static entity", &(entity.clone() + &read("static_entity.wgsl")), None);
    let oit = read("oit.wgsl");
    ok &= check("oit world", &(terrain.clone() + &oit + &read("oit_world.wgsl")), None);
    ok &= check("oit clouds", &(entity.clone() + &oit + &read("oit_clouds.wgsl")), None);
    ok &= check("oit features", &(entity.clone() + &oit + &read("oit_features.wgsl")), None);
    ok &= check(
        "oit border",
        &(read("border.wgsl") + &oit + &read("oit_border.wgsl")),
        None,
    );
    ok &= check("visbuffer", &(terrain.clone() + &read("visbuffer.wgsl")), None);
    // Shaders with HDR's shared maths in front (gfx/custom_sky.rs, stars.rs, hdr.rs).
    let hdr_light = read("hdr_light.wgsl");
    ok &= check("custom sky", &(hdr_light.clone() + &read("gfx/custom_sky.wgsl")), None);
    ok &= check("stars", &(hdr_light.clone() + &read("gfx/stars.wgsl")), None);
    ok &= check("hdr", &(hdr_light.clone() + &read("hdr.wgsl")), None);
    ok &= check(
        "gpu occlusion",
        &(read("gfx/gpu_culling.wgsl") + &read("gfx/gpu_occlusion.wgsl")),
        None,
    );
    for file in std::env::args().skip(1) {
        ok &= check(&file, &read(&file), None);
    }
    std::process::exit(if ok { 0 } else { 1 });
}
'''


def main():
    harness = ROOT / '.local/android/wgsl-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    (harness / 'Cargo.toml').write_text(f'''[package]
name = "android-wgsl-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
naga = {{ path = "{(ROOT / 'third_party/naga').as_posix()}", features = ["wgsl-in", "glsl-out"] }}
''', encoding='utf-8')
    # The workspace's lock: naga's dependencies at the versions the game builds with.
    shutil.copyfile(ROOT / 'Cargo.lock', harness / 'Cargo.lock')
    (harness / 'src/main.rs').write_text(MAIN.replace('@RENDER@', RENDER.as_posix()),
                                         encoding='utf-8')
    standalone = sorted(
        path.relative_to(RENDER).as_posix() for path in RENDER.rglob('*.wgsl')
        if path.name not in FRAGMENTS and path.name != 'terrain.wgsl'
    )
    subprocess.run(['cargo', 'run', '--quiet', '--manifest-path', str(harness / 'Cargo.toml'),
                    '--', *standalone], cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
