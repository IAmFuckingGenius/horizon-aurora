# Android performance reports

In the launcher, open Settings, enable **Performance recording**, then press
Play. Reproduce the slow startup, travel or other problem during normal play.
Return to the launcher and select **Performance report → Open in Files / Reports**.
Copy or share the `.jsonl` file from **Horizon Android → Reports** in system Files.

Recording is off by default. Each run is limited to 30 minutes and 8 MiB; the
three latest internal reports are retained. Opening Reports does not create copies.
Copies exported by the user remain in their chosen destination. Older versions'
copies in Download/HorizonAndroid are left untouched.
Reports are written on a bounded background queue and flushed once per sample;
if Android kills the process, completed samples remain available on next launch.
No general storage permission is needed to browse the document provider.

The report contains:

- One-second frame/CPU/GPU summaries with percentiles, maxima and slow events.
- Named render/server CPU spans: elapsed and thread CPU time, counts and maxima.
- Worker tasks: saved-chunk loading, generation, decoration, lighting, meshing
  and saving; concurrent task totals are reported separately from frame timings.
- Streaming queues, dirty light, pending mesh uploads and cumulative upload bytes.
- Terrain draw calls, resident sections, GPU allocation counters where available,
  resolution, render/simulation distances, presentation mode and effective FPS cap.
- Focus/pause/idle state, underwater state and entity/particle counts.
- Camera yaw/pitch and player velocity for distinguishing a changed view or movement
  from a stationary scene. Player coordinates remain excluded.
- Buffer creation, error-scope resolution and arena growth-copy CPU timings.
- The source commit and tracked-change flag when built with `android_build.py`,
  so reports from different builds with the same version can be distinguished.
- Android power-saving mode and reported thermal status (Android 10+), without
  root or access to device identifiers. A thermal status of 0 means none, 1 light,
  2 moderate, 3 severe, 4 critical, 5 emergency, 6 shutdown; unavailable is null.

These identify expensive stages and correlate spikes with background work. Nested
spans overlap and must not be added together. Wall time exceeding thread CPU time
can mean scheduling or waiting; it does not prove a particular lock is responsible.
GPU timing is asynchronous and can be missing; this is not a per-shader GPU profiler.
Recording itself adds timing overhead, so compare benchmark runs with recording off.

There is no automatic upload. Reports do not include account tokens, nicknames,
chat, server addresses, world names, seeds or player coordinates.

Developer regression check: `python tools/test_android_performance.py`. It runs
the actual recorder/statistics/timing code in a host harness and substitutes only
Android publication. The isolated storage probe tests provider access separately.

## Vulkan terrain validation

Android Vulkan separates proven opaque terrain from alpha-cutout terrain within
the same render pass. The opaque shader does not discard fragments. This lets
mobile GPUs reject more hidden pixels before shading them; see the
[Khronos tile-rendering guidance](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html).
Resolution, textures, fog, lighting and render distance are unchanged.

Meshes with order-sensitive overlapping surfaces retain the ordinary shader and
ordering. Fading sections use the cutout path. Shader packs, visible distant
terrain and GPU occlusion culling retain their existing render path. OpenGL ES
retains its previous mesh order and shaders.

On a 2405CPCFBG tablet with Adreno 710, Vulkan Compatibility at 2560×1600 and
16 chunks was measured in a stationary, frozen test world with recording,
extended GPU profiling and ADPF disabled. Alternating runs used the same camera
and resident geometry, without pending chunk work:

| Lighting | Ordinary terrain | Split terrain | GPU time reduction |
| --- | ---: | ---: | ---: |
| Night, three runs per path | 18.79 ms | 13.63 ms | 27% |
| Day, two runs per path | 19.79 ms | 14.77 ms | 25% |

These results describe this fixture and device, not a general FPS guarantee.
VSync limited the faster night runs to 60 FPS. The authenticated developer
control command `terrain-passes single|split` switches the pipeline for A/B
measurement; it does not change graphics settings or rebuild meshes. Status and
performance reports expose `terrain_pass_draws` as opaque/cutout/order-sensitive
draw counts, or null when the ordinary path is active.

The terrain shader regression tests compare the output of both paths over alpha
thresholds, fog, section fading and emission tiers. Geometry tests preserve the
ordering of overlapping cutout/opaque faces. This does not replace testing on
other GPUs, shader packs and resource packs.

## Vulkan sky ordering

Without a shader pack, Android Vulkan can draw the vanilla Overworld sky after
solid terrain, before entities and translucent surfaces, within the same render
pass. The sky reads depth at the far plane without writing it. This avoids shading
pixels hidden by terrain; see the [skybox ordering discussion from NVIDIA](https://developer.nvidia.com/gpugems/gpugems/part-v-performance-and-practicalities/chapter-28-graphics-pipeline-performance).
The sky fragment shaders, time, colours, fog and celestial rotation are unchanged.

Native and classic shader packs retain their existing ordering. The title screen,
End sky, water mirror and GPU occlusion path also retain their original path.
OpenGL ES is unchanged. Failure to create any of the additional pipelines retains
the original sky order. The authenticated developer command `sky-order early|late`
allows comparison without changing the world's settings or network protocol.

On the same Adreno 710 tablet at 2560×1600 and 16 chunks, with split terrain active:

| Test | Early sky | Late sky |
| --- | ---: | ---: |
| Stationary GPU time, mean of three runs | 14.73 ms | 13.59 ms |
| GPU time while turning, two runs | 16.47–16.58 ms | 13.43–13.49 ms |
| Frame p99 while turning | 21.17–21.40 ms | 18.45–18.91 ms |

The turning test used running world time near sunrise and the same eight-second
touch swipe from the same initial camera direction. It is a separate workload
from the stationary comparison; its percentages must not be multiplied with the
earlier terrain results to claim a combined speedup.

Paired daytime, sunset, night and sunrise captures had identical RGB pixels outside
the changing diagnostic counters. Golden Hour was enabled and disabled, and its
captures also matched between the two sky-order requests. This verifies that pack
on this device, not every shader pack or GPU. Capture tools and captures are not
included in the APK or repository.

## Vulkan Batching: indirect ranges and the vanilla threshold

The opaque/cutout split initially passed each material group's first draw as the
base of its indirect buffer. Those groups actually share the complete frame
buffer. Direct draws ignored that parameter, hiding the bug in Compatibility;
indirect draws could replay the wrong commands and omit visible surfaces when
culling changed paths. Both material subranges now retain buffer base zero.

Android's selected Batching mode also now uses the existing flat-index path for
ordinary terrain above its 512-section threshold. Previously the Android
threshold was defined but the vanilla backend policy still disabled it, leaving
the slower per-section indirect calls. Native shader materials retain their
previous path; classic terrain/shadow programs retain their explicit flat inputs.
Neither this selection nor the offset fix changes the default Compatibility mode.

On the Adreno 710 tablet at 2560×1600, 16 chunks, seed
`-4240118709374351666`, position `-24.5 102 -47.5`, with world time frozen and
resident geometry, a same-build developer A/B comparison with culling active gave:

| Terrain commands | GPU mean | Mean FPS |
| --- | ---: | ---: |
| Per-section indirect | 46.05 ms | 20.09 |
| Flattened index batches | 22.30 ms | 38.37 |

Four camera directions were compared with direct draws after the offset fix;
the large missing regions were gone. This is a device/scene-specific comparison,
not a performance guarantee or proof that every reported Batching glitch has the
same cause. Final automatic-path validation is recorded separately from forced
developer A/B runs.

The resulting optimized test APK's automatic Batching path measured 38.58 FPS
and 22.29 ms mean GPU time at that same spawn, without forcing the flat-index
switch. A ten-second camera flight through the area measured 31.90 FPS and
27.00 ms mean GPU time; movement and newly exposed geometry remain more costly.
ARM64 Clippy, ARMv7 compile checks and four flat-index planning/GPU tests passed.
Golden Hour was enabled in-world and disabled again on this APK; this is not a
validation of every native or classic shader pack.

## 2026-11-03: rain cost, thermal governor, leaving the app, Horizon Boost

### Leaving the app saves the world

Android stops delivering frames once it pauses the activity (Home, the app
switcher, a document picker over the game). The pause and the server-thread
notification were both published from the frame, so single player kept
simulating in the background and nothing was saved until the next 5-minute
autosave; swiping the app away from recents then lost that time. The event loop
also polled without waiting while no frames came.

Now the window's suspension, or frames stopping while the window has no focus,
opens the pause menu, tells the server thread at once and asks it for a full save
that waits for the region writer, once per stay in the background. The pause's own
save is skipped when that save has just run. While no frames come, the loop waits
50 ms between iterations. Multiplayer and LAN guests have nothing to save locally;
a world open to LAN is saved but keeps running, as in vanilla.

Desktop check with the developer command `app-background`, which runs the same
code: place a block and give items, then `kill -9` the game and reopen the world.
With the command, the block, the items and the player position were kept; without
it, all three were lost. Android's lifecycle callbacks themselves were not run on a
device for this change.

### Rain and snow on the GPU

Rain is a camera-facing quad per column within the weather radius (21 × 21 columns
at radius 10), up to 20 blocks tall. About 97% of `rain.png`'s texels are
transparent, so the cost is almost entirely fragments that look up one texel and
discard: roughly five to six screen layers in open rain. Particles, sky darkening
and fog changes did not measurably add GPU time.

The classic pass now draws weather through a shader with only the inputs it reads,
which computes the sprite's texel rectangle once per vertex instead of per pixel.
With ticks frozen, frames were bit-identical to the previous shader (0 differing
pixels at 1920 × 1013). `weather-shader classic|lean` switches between them.
Shader packs keep their own weather path.

Desktop measurements, AMD Radeon 780M (RADV, Vulkan), 1920 × 1013 offscreen,
uncapped, standing in plains rain at seed 1234 (−960 120 80), mean of three
4-second runs. Each configuration is a separate game; "added" is rain minus clear
in the same game:

| Configuration | Clear GPU | Rain GPU | Added by rain |
| --- | ---: | ---: | ---: |
| Previous shader (alternating A/B, same game) | 0.94 ms | 1.27 ms | 0.33 ms |
| Lean shader (alternating A/B, same game) | 0.94 ms | 1.19 ms | 0.25 ms |
| Lean shader, vanilla settings | 0.91 ms | 1.14 ms | 0.23 ms |
| Rain density 50% | 0.89 ms | 1.04 ms | 0.15 ms |
| Rain density 25% | 0.92 ms | 0.99 ms | 0.07 ms |
| Weather distance 5 (vanilla's Fast radius) | 0.93 ms | 1.09 ms | 0.15 ms |
| Weather distance 5 and density 50% | 0.95 ms | 1.02 ms | 0.06 ms |
| No rain splashes | 0.93 ms | 1.19 ms | 0.26 ms |

Splash particles cost CPU time on the client, not GPU time; their phone CPU cost
was not measured. On the RTX 4060 Laptop GPU rain added 0.33 ms with the previous
shader. These are desktop numbers; the phone's 26 ms rain frames (from the user's
report, S21 FE, 2340 × 1080) need the phone measurement below. A tile-based mobile
GPU is expected to be fragment-bound in the same way, but no percentage is claimed
for it.

Not done: clipping column quads to the texture's non-empty texel columns (about a
fifth fewer rain fragments, at the cost of instanced geometry) and drawing weather
at a lower resolution.

### Thermal governor

Horizon Boost's **Adapt to heat** (on by default) reads `AThermal_getCurrentThermalStatus`
(API 30, once a second) and `AThermal_getThermalHeadroom` (API 31, every 2 s, 10 s
forecast), resolved with `dlsym`, or `PowerManager.getCurrentThermalStatus` through
JNI on API 29. Levels:

| Level | Entered at | Frame cap | Chunk uploads/frame | Render scale |
| --- | --- | ---: | ---: | ---: |
| 0 | status none/light and headroom < 0.75 | unchanged | unchanged | unchanged |
| 1 | moderate, or headroom ≥ 0.75 | 60 | unchanged | unchanged |
| 2 | severe, or headroom ≥ 0.85 | 45 | 4 | unchanged |
| 3 | critical or above, or headroom ≥ 0.95 | 30 | 2 | −20 points, ≥ 50% |

A level rises at once and falls one step after readings have allowed it for 30 s
(lower thresholds 0.65/0.78/0.88). A cap never raises the player's own lower cap.
Status, headroom, level and caps are in `status` (`thermal`), in performance
reports (`context.boost_thermal`) and, with the performance overlay on, as its last
line. `thermal-test STATUS [HEADROOM]|off` injects readings: on desktop the measured
frame rate went from unlimited to 59.8, 44.9 and 30.0 FPS at levels 1–3, and the
world resolution to 80%.

### Memory pressure

`onLowMemory` (winit's memory warning) and new trim levels read from
`ActivityManager.getMyMemoryState` (every 5 s in play, 2 s in the background; acted
on from RUNNING_LOW in play and from BACKGROUND when away) drop decoded sounds,
pooled mesh buffers and the staging belts, and return free allocator pages. At
RUNNING_CRITICAL or onLowMemory, LOD distance (when set) is held at half for three
minutes. Performance reports count trims (`context.lifecycle`). Not verified on a
device under real memory pressure.

### Horizon Boost

Horizon Boost is a settings sheet in the engine, opened with **Horizon Boost...** in
Video Settings while in a world (or `boost open`). It is not a bundled native mod:
the bundled modules are pinned third-party sources wired to fixed hook points, whose
settings pages are button lists, and they cannot reach the renderer's animation
masks, weather columns, frame limiter or thermal governor without new engine APIs.
The sheet reuses the existing settings (vanilla options and `horizon-graphics.txt`)
and adds rain density, rain splash share, stars, sun and moon, edge-fog start and
switches for water, lava, fire, nether portal and other block animations. Every
default draws what vanilla draws; the Performance preset halves rain columns and
splashes. Animations already update only for layers visible on screen.

Not implemented: dynamic resolution driven by GPU time.

### Phone measurement

1. Install the build with launcher **Performance recording** on and ADPF/GPU
   profiling as usual; note thermal status before starting (cool device).
2. Rain cost: fixed spot in open plains, `/weather rain`, `/gamerule advance_time
   false`, camera still for 60 s. Record GPU ms; repeat with `/weather clear`. Then,
   through the developer control socket, `weather-shader classic` and `lean`, 60 s
   each, alternating twice. Then Boost → Quality: Rain density 50% and 25%, Weather
   distance 5, 60 s each.
3. Thermal: Boost → Performance → Adapt to heat on, overlay on. Play 15 minutes in
   rain without a frame cap; record when the overlay's thermal line and cap change,
   FPS and GPU ms. Repeat with Adapt to heat off from a cool start.
4. Leaving the app: in a single-player world place a block and pick up items, press
   Home, wait 2 s, swipe the app away from recents, reopen the world: the block,
   items and position must be there. Also with a document picker or share sheet open
   over the game. Logcat should show `saved the world for the background`.
5. Memory: with LOD distance on, open several heavy apps, return, and check
   `context.lifecycle.memory_trims` in the report and that no chunks are missing.
