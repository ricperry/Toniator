# JPEG XL decoder dependency notices

Original license files are copied verbatim from the locked crates.io packages,
except alloc-stdlib as explained below. The decoder is pure Rust; libjxl/lcms2
are used only to generate synthetic test fixtures.

| Package | License | Registry checksum |
| --- | --- | --- |
| alloc-no-stdlib-2.0.4 | BSD-3-Clause | `cc7bb162ec39d46ab1ca8c77bf72e890535becd1751bb45f64c597edb4c8c6b3` |
| alloc-stdlib-0.2.4 | BSD-3-Clause | `0e76a019e91224d279006ff972f1e984179a6e9feb050adba6ce8274aef23195` |
| brotli-decompressor-5.0.3 | BSD-3-Clause/MIT | `3a32acac15fe1967bc3986b2a6347dffc965602354ea6f450ad07e8bfd253583` |
| jxl-bitstream-1.1.0 | MIT OR Apache-2.0 | `b480e752277e29eb4054f69546887a9b84656fe78c08f54ba5850ced98a378fe` |
| jxl-coding-1.0.1 | MIT OR Apache-2.0 | `cd972bcd125e776f1eb241ac50e39f956095a1c2770c64736c968f8946bd9a3c` |
| jxl-color-0.11.0 | MIT OR Apache-2.0 | `f316b1358c1711755b3ee8e8cb5c4a1dad12e796233088a7a513440782de80b2` |
| jxl-frame-0.13.3 | MIT OR Apache-2.0 | `2d967c6fd669c7c01060b5022d8835fa82fd46b06ffc98b549f17600a097c2b3` |
| jxl-grid-0.6.2 | MIT OR Apache-2.0 | `01671307879a033bfa52e6e8784b941aca770b3f3a7d33830b455b6844f793fb` |
| jxl-image-0.13.0 | MIT OR Apache-2.0 | `c5f752d62577c702a94dbbce4045caf08cb58639e8a4d56464b40ecf33ffe565` |
| jxl-jbr-0.2.1 | MIT OR Apache-2.0 | `e35d032bcec660647828527ff42c6f5776d2fd44b8357f9f6d9ac6dc07218e46` |
| jxl-modular-0.11.3 | MIT OR Apache-2.0 | `2a2f045b24c738dd91d482be385512b512721ae08a671bd4b27bf1c47f215235` |
| jxl-oxide-0.12.6 | MIT OR Apache-2.0 | `d36c662923f47586880211f3bc7c0d83fb3a9b410d278c7bde93450748abeef3` |
| jxl-oxide-common-1.0.0 | MIT OR Apache-2.0 | `b62394c5021b3a9e7e0dbb2d639d555d019090c9946c39f6d3b09d390db4157b` |
| jxl-render-0.12.4 | MIT OR Apache-2.0 | `d34386bfdb6a19b5a30cc9beb4d475d537422c31ae8c39bb69640fcce3fcaf19` |
| jxl-threadpool-1.0.0 | MIT OR Apache-2.0 | `25f15eb830aa77a7f21148d72e153562a26bfe570139bd4922eab1908dd499d3` |
| jxl-vardct-0.11.1 | MIT OR Apache-2.0 | `ce72a18c6d3a47172ab6c479be2bdb56f22066b5d7092663f03b4490820b4511` |
| tracing-0.1.44 | MIT | `63e71662fa4b2a2c3a26f570f037eb95bb1f85397f3cd8076caed2f026a6d100` |
| tracing-core-0.1.36 | MIT | `db97caf9d906fbde555dd62fa95ddba9eecfd14cb388e4f491a66d74cd5fb79a` |

alloc-stdlib-0.2.4 omits its license file in the registry archive. Its LICENSE is retrieved from
`https://raw.githubusercontent.com/dropbox/rust-alloc-no-stdlib/master/LICENSE` (same repository as alloc-no-stdlib); SHA-256 `c0c56f26d9c051cac4d200c34c84e7ae9aaa853e01a982a1df08b09931e518ae`.
