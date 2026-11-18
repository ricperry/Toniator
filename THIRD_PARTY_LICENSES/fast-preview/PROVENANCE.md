# Fast preview dependency notices

These original license texts cover the 82 newly locked dependencies resolved for
the Linux x86_64 default app build, including shader/compiler and build-time
dependencies. Inclusion here does not imply every package is linked into the
runtime executable. Existing JPEG XL and vendored geometry notices remain in
their separate directories. This inventory excludes the standalone GPU evaluation tool.

Vello `vello_common`, `vello_gpu`, and `vello_gpu_shaders` use unmodified upstream
revision `72c2fb0628dc6f4bbc7eaf3375284be89be03a06` from
[linebender/vello](https://github.com/linebender/vello/tree/72c2fb0628dc6f4bbc7eaf3375284be89be03a06).
The production consumer uses wgpu 30.0.0 with Vulkan enabled. Cargo.lock identifies
all registry checksums and git revisions; no standalone evaluation patches are applied.

License files are copied verbatim from their locked package directories except
`profiling`, `spirv`, `wesl`, `wesl-macros`, `wgsl-parse` and `wgsl-types`, whose
registry archives omit them. Those texts come from the exact upstream revisions
recorded by each package archive in `.cargo_vcs_info.json`. The accompanying
[manifest](manifest.json) records pinned URLs, original source identities and
SHA-256 hashes for every retained text; it contains no machine-local paths.

Both Linux payloads ship this directory under
`share/licenses/io.github.ricperry.Toniator/fast-preview/`.

| Package | Declared license |
| --- | --- |
| aho-corasick 1.1.5 | Unlicense OR MIT |
| annotate-snippets 0.12.17 | MIT OR Apache-2.0 |
| ascii-canvas 4.0.0 | Apache-2.0/MIT |
| ash 0.38.0+1.3.281 | MIT OR Apache-2.0 |
| bit-set 0.10.0 | Apache-2.0 OR MIT |
| bit-set 0.8.0 | Apache-2.0 OR MIT |
| bit-vec 0.8.0 | Apache-2.0 OR MIT |
| bit-vec 0.9.1 | Apache-2.0 OR MIT |
| codespan-reporting 0.13.1 | Apache-2.0 |
| color 0.3.3 | Apache-2.0 OR MIT |
| convert_case 0.10.0 | MIT |
| derive_more 2.1.1 | MIT |
| derive_more-impl 2.1.1 | MIT |
| document-features 0.2.12 | MIT OR Apache-2.0 |
| ena 0.14.4 | MIT OR Apache-2.0 |
| fearless_simd 1.2.0 | Apache-2.0 OR MIT |
| fearless_simd_macros 0.1.0 | Apache-2.0 OR MIT |
| fixedbitset 0.5.7 | MIT OR Apache-2.0 |
| fnv 1.0.7 | Apache-2.0 / MIT |
| gpu-allocator 0.28.0 | MIT OR Apache-2.0 |
| guillotiere 0.7.0 | MIT/Apache-2.0 |
| itertools 0.14.0 | MIT OR Apache-2.0 |
| keccak 0.1.6 | Apache-2.0 OR MIT |
| lalrpop 0.22.2 | Apache-2.0 OR MIT |
| lalrpop-util 0.22.2 | Apache-2.0 OR MIT |
| lexical 7.0.5 | MIT/Apache-2.0 |
| lexical-core 1.0.6 | MIT/Apache-2.0 |
| lexical-parse-float 1.0.6 | MIT/Apache-2.0 |
| lexical-parse-integer 1.0.6 | MIT/Apache-2.0 |
| lexical-util 1.0.7 | MIT/Apache-2.0 |
| lexical-write-float 1.0.6 | MIT/Apache-2.0 |
| lexical-write-integer 1.0.6 | MIT/Apache-2.0 |
| libloading 0.8.9 | ISC |
| linebender_resource_handle 0.1.1 | Apache-2.0 OR MIT |
| litrs 1.0.1 | MIT OR Apache-2.0 |
| lock_api 0.4.14 | MIT OR Apache-2.0 |
| logos 0.16.1 | MIT OR Apache-2.0 |
| logos-codegen 0.16.1 | MIT OR Apache-2.0 |
| logos-derive 0.16.1 | MIT OR Apache-2.0 |
| naga 30.0.0 | MIT OR Apache-2.0 |
| naga-types 30.0.1 | MIT OR Apache-2.0 |
| new_debug_unreachable 1.0.6 | MIT |
| ordered-float 5.5.1 | MIT |
| parking_lot 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core 0.9.12 | MIT OR Apache-2.0 |
| peniko 0.6.1 | Apache-2.0 OR MIT |
| petgraph 0.7.1 | MIT OR Apache-2.0 |
| phf_shared 0.11.3 | MIT |
| precomputed-hash 0.1.1 | MIT |
| presser 0.3.1 | MIT OR Apache-2.0 |
| profiling 1.0.18 | MIT OR Apache-2.0 |
| raw-window-handle 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| regex 1.13.1 | MIT OR Apache-2.0 |
| regex-automata 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax 0.8.11 | MIT OR Apache-2.0 |
| renderdoc-sys 1.1.0 | MIT OR Apache-2.0 |
| rustc-hash 1.1.0 | Apache-2.0/MIT |
| rustversion 1.0.23 | MIT OR Apache-2.0 |
| same-file 1.0.6 | Unlicense/MIT |
| scopeguard 1.2.0 | MIT OR Apache-2.0 |
| sha3 0.10.9 | MIT OR Apache-2.0 |
| spirv 0.4.0+sdk-1.4.341.0 | Apache-2.0 |
| string_cache 0.8.9 | MIT OR Apache-2.0 |
| syn 3.0.4 | MIT OR Apache-2.0 |
| term 1.2.1 | MIT OR Apache-2.0 |
| unicode-segmentation 1.13.3 | MIT OR Apache-2.0 |
| unicode-width 0.2.3 | MIT OR Apache-2.0 |
| unicode-xid 0.2.6 | MIT OR Apache-2.0 |
| vello_common 0.3.0 | Apache-2.0 OR MIT |
| vello_gpu 0.3.0 | Apache-2.0 OR MIT |
| vello_gpu_shaders 0.3.0 | Apache-2.0 OR MIT |
| walkdir 2.5.0 | Unlicense/MIT |
| wesl 0.4.0 | MIT OR Apache-2.0 |
| wesl-macros 0.4.0 | MIT OR Apache-2.0 |
| wgpu 30.0.0 | MIT OR Apache-2.0 |
| wgpu-core 30.0.0 | MIT OR Apache-2.0 |
| wgpu-core-deps-windows-linux-android 30.0.1 | MIT OR Apache-2.0 |
| wgpu-hal 30.0.0 | MIT OR Apache-2.0 |
| wgpu-naga-bridge 30.0.1 | MIT OR Apache-2.0 |
| wgpu-types 30.0.1 | MIT OR Apache-2.0 |
| wgsl-parse 0.4.0 | MIT OR Apache-2.0 |
| wgsl-types 0.4.0 | MIT OR Apache-2.0 |
