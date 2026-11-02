# Third-party notices

Bible MCP bundles or depends on the following.

## Data

- **World English Bible (WEB)** text: public domain.
- **Cross-references** from OpenBible.info (https://www.openbible.info/labs/cross-references/),
  licensed CC-BY 4.0 (https://creativecommons.org/licenses/by/4.0/). Votes are used to rank
  and filter the references.

## Model

- **nomic-embed-text-v1.5** (https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) by Nomic AI,
  Apache License 2.0. The stored verse vectors were produced with it, and queries are embedded
  with it locally or through a hosted provider.

## Bundled runtime

- **ONNX Runtime** (https://github.com/microsoft/onnxruntime), MIT License,
  Copyright (c) Microsoft Corporation. Release archives ship its shared library and its own
  license files next to the `bible-mcp` executable.

## Rust dependencies

Each crate is distributed under the license shown; full license texts are in the crate sources.

| Crate | Version | License |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.4 | Unlicense OR MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| android_system_properties | 0.1.5 | MIT/Apache-2.0 |
| anstream | 1.0.0 | MIT OR Apache-2.0 |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 |
| anyhow | 1.0.102 | MIT OR Apache-2.0 |
| async-trait | 0.1.92 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| axum | 0.8.9 | MIT |
| axum-core | 0.5.6 | MIT |
| base64 | 0.13.1 | MIT/Apache-2.0 |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| bitflags | 2.13.0 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.11.1 | MIT |
| castaway | 0.2.4 | MIT |
| cc | 1.2.63 | MIT OR Apache-2.0 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | MIT |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 |
| chrono | 0.4.45 | MIT OR Apache-2.0 |
| clap | 4.6.1 | MIT OR Apache-2.0 |
| clap_builder | 4.6.0 | MIT OR Apache-2.0 |
| clap_derive | 4.6.1 | MIT OR Apache-2.0 |
| clap_lex | 1.1.0 | MIT OR Apache-2.0 |
| codespan-reporting | 0.13.1 | Apache-2.0 |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 |
| compact_str | 0.9.1 | MIT |
| console | 0.15.11 | MIT |
| console | 0.16.3 | MIT |
| cookie | 0.18.1 | MIT OR Apache-2.0 |
| cookie_store | 0.22.1 | MIT OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc32fast | 1.5.0 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.6 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.18 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.21 | MIT OR Apache-2.0 |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| cxx | 1.0.205 | MIT OR Apache-2.0 |
| cxx-build | 1.0.205 | MIT OR Apache-2.0 |
| cxxbridge-flags | 1.0.205 | MIT OR Apache-2.0 |
| cxxbridge-macro | 1.0.205 | MIT OR Apache-2.0 |
| darling | 0.20.11 | MIT |
| darling | 0.24.1 | MIT |
| darling_core | 0.20.11 | MIT |
| darling_core | 0.24.1 | MIT |
| darling_macro | 0.20.11 | MIT |
| darling_macro | 0.24.1 | MIT |
| dary_heap | 0.3.9 | MIT OR Apache-2.0 |
| deranged | 0.5.8 | MIT OR Apache-2.0 |
| derive_builder | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_core | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_macro | 0.20.2 | MIT OR Apache-2.0 |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| dirs | 5.0.1 | MIT OR Apache-2.0 |
| dirs | 6.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.4.1 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| displaydoc | 0.2.6 | MIT OR Apache-2.0 |
| document-features | 0.2.12 | MIT OR Apache-2.0 |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 |
| either | 1.16.0 | MIT OR Apache-2.0 |
| encode_unicode | 1.0.0 | Apache-2.0 OR MIT |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| esaxx-rs | 0.1.10 | Apache-2.0 |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 |
| fastembed | 5.16.0 | Apache-2.0 |
| fastrand | 2.4.1 | Apache-2.0 OR MIT |
| find-msvc-tools | 0.1.9 | MIT OR Apache-2.0 |
| flate2 | 1.1.9 | MIT OR Apache-2.0 |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| foldhash | 0.1.5 | Zlib |
| foldhash | 0.2.0 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| futures | 0.3.32 | MIT OR Apache-2.0 |
| futures-channel | 0.3.32 | MIT OR Apache-2.0 |
| futures-core | 0.3.32 | MIT OR Apache-2.0 |
| futures-executor | 0.3.32 | MIT OR Apache-2.0 |
| futures-io | 0.3.32 | MIT OR Apache-2.0 |
| futures-macro | 0.3.32 | MIT OR Apache-2.0 |
| futures-sink | 0.3.32 | MIT OR Apache-2.0 |
| futures-task | 0.3.32 | MIT OR Apache-2.0 |
| futures-util | 0.3.32 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.2 | MIT OR Apache-2.0 |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hashlink | 0.9.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hf-hub | 0.5.0 | Apache-2.0 |
| http | 1.4.1 | MIT OR Apache-2.0 |
| http-body | 1.0.1 | MIT |
| http-body-util | 0.1.3 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hyper | 1.10.1 | MIT |
| hyper-rustls | 0.27.9 | Apache-2.0 OR ISC OR MIT |
| hyper-util | 0.1.20 | MIT |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 |
| iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 |
| icu_collections | 2.2.0 | Unicode-3.0 |
| icu_locale_core | 2.2.0 | Unicode-3.0 |
| icu_normalizer | 2.2.0 | Unicode-3.0 |
| icu_normalizer_data | 2.2.0 | Unicode-3.0 |
| icu_properties | 2.2.0 | Unicode-3.0 |
| icu_properties_data | 2.2.0 | Unicode-3.0 |
| icu_provider | 2.2.0 | Unicode-3.0 |
| id-arena | 2.3.0 | MIT/Apache-2.0 |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| indexmap | 2.14.0 | Apache-2.0 OR MIT |
| indicatif | 0.17.11 | MIT |
| indicatif | 0.18.4 | MIT |
| ipnet | 2.12.0 | MIT OR Apache-2.0 |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| js-sys | 0.3.99 | MIT OR Apache-2.0 |
| leb128fmt | 0.1.0 | MIT OR Apache-2.0 |
| libc | 0.2.186 | MIT OR Apache-2.0 |
| libloading | 0.9.0 | ISC |
| libredox | 0.1.17 | MIT |
| libsqlite3-sys | 0.28.0 | MIT |
| link-cplusplus | 1.0.12 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.2 | Unicode-3.0 |
| litrs | 1.0.0 | MIT OR Apache-2.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.32 | MIT OR Apache-2.0 |
| lru-slab | 0.1.3 | MIT OR Apache-2.0 OR Zlib |
| macro_rules_attribute | 0.2.2 | Apache-2.0 OR MIT OR Zlib |
| macro_rules_attribute-proc_macro | 0.2.2 | Apache-2.0 OR MIT OR Zlib |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| matrixmultiply | 0.3.10 | MIT/Apache-2.0 |
| memchr | 2.8.1 | Unlicense OR MIT |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| minimal-lexical | 0.2.1 | MIT/Apache-2.0 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.1 | MIT |
| monostate | 0.1.18 | MIT OR Apache-2.0 |
| monostate-impl | 0.1.18 | MIT OR Apache-2.0 |
| ndarray | 0.17.2 | MIT OR Apache-2.0 |
| nix | 0.31.3 | MIT |
| nom | 7.1.3 | MIT |
| num-complex | 0.4.6 | MIT OR Apache-2.0 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-integer | 0.1.46 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| number_prefix | 0.4.0 | MIT |
| numkong | 7.8.7 | Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| onig | 6.5.3 | MIT |
| onig_sys | 69.9.3 | MIT |
| option-ext | 0.2.0 | MPL-2.0 |
| ort | 2.0.0-rc.12 | MIT OR Apache-2.0 |
| ort-sys | 2.0.0-rc.12 | MIT OR Apache-2.0 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| paste | 1.0.15 | MIT OR Apache-2.0 |
| pastey | 0.2.3 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkg-config | 0.3.33 | MIT OR Apache-2.0 |
| portable-atomic | 1.13.1 | Apache-2.0 OR MIT |
| portable-atomic-util | 0.2.7 | Apache-2.0 OR MIT |
| potential_utf | 0.1.5 | Unicode-3.0 |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| prettyplease | 0.2.37 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.106 | MIT OR Apache-2.0 |
| process-wrap | 10.0.1 | Apache-2.0 OR MIT |
| quinn | 0.11.12 | MIT OR Apache-2.0 |
| quinn-proto | 0.11.19 | MIT OR Apache-2.0 |
| quinn-udp | 0.5.16 | MIT OR Apache-2.0 |
| quote | 1.0.45 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.10.3 | MIT OR Apache-2.0 |
| rand | 0.9.4 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.10.1 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 |
| rawpointer | 0.2.1 | MIT/Apache-2.0 |
| rayon | 1.12.0 | MIT OR Apache-2.0 |
| rayon-cond | 0.4.0 | Apache-2.0/MIT |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| redox_users | 0.4.6 | MIT |
| redox_users | 0.5.2 | MIT |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 |
| regex | 1.12.3 | MIT OR Apache-2.0 |
| regex-automata | 0.4.14 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.10 | MIT OR Apache-2.0 |
| reqwest | 0.12.28 | MIT OR Apache-2.0 |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rmcp | 3.8.0 | Apache-2.0 |
| rmcp-macros | 3.8.0 | Apache-2.0 |
| rusqlite | 0.31.0 | MIT |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | 0.23.40 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.14.1 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.13 | ISC |
| rustversion | 1.0.22 | MIT OR Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| safetensors | 0.7.0 | Apache-2.0 |
| schemars | 1.2.3 | MIT |
| schemars_derive | 1.2.3 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| scratch | 1.0.9 | MIT OR Apache-2.0 |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.228 | MIT OR Apache-2.0 |
| serde_core | 1.0.228 | MIT OR Apache-2.0 |
| serde_derive | 1.0.228 | MIT OR Apache-2.0 |
| serde_derive_internals | 0.30.0 | MIT OR Apache-2.0 |
| serde_json | 1.0.150 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.9 | MIT |
| slab | 0.4.12 | MIT |
| smallvec | 1.15.1 | MIT OR Apache-2.0 |
| socket2 | 0.6.4 | MIT OR Apache-2.0 |
| socks | 0.3.4 | MIT/Apache-2.0 |
| spm_precompiled | 0.1.4 | Apache-2.0 |
| sqlite-vec | 0.1.9 | MIT/Apache-2.0 |
| sse-stream | 0.2.6 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| syn | 2.0.117 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.13.2 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| termcolor | 1.4.1 | Unlicense OR MIT |
| thiserror | 1.0.69 | MIT OR Apache-2.0 |
| thiserror | 2.0.18 | MIT OR Apache-2.0 |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.18 | MIT OR Apache-2.0 |
| time | 0.3.47 | MIT OR Apache-2.0 |
| time-core | 0.1.8 | MIT OR Apache-2.0 |
| time-macros | 0.2.27 | MIT OR Apache-2.0 |
| tinystr | 0.8.3 | Unicode-3.0 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tokenizers | 0.22.2 | Apache-2.0 |
| tokio | 1.52.3 | MIT |
| tokio-macros | 2.7.0 | MIT |
| tokio-rustls | 0.26.4 | MIT OR Apache-2.0 |
| tokio-stream | 0.1.19 | MIT |
| tokio-util | 0.7.18 | MIT |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| try-lock | 0.2.5 | MIT |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-normalization-alignments | 0.1.12 | MIT/Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unicode-width | 0.2.2 | MIT OR Apache-2.0 |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 |
| unicode_categories | 0.1.1 | MIT OR Apache-2.0 |
| unit-prefix | 0.5.2 | MIT |
| untrusted | 0.9.0 | ISC |
| ureq | 3.3.0 | MIT OR Apache-2.0 |
| ureq-proto | 0.6.0 | MIT OR Apache-2.0 |
| url | 2.5.8 | MIT OR Apache-2.0 |
| usearch | 2.26.6 | Apache-2.0 |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT |
| uuid | 1.29.0 | Apache-2.0 OR MIT |
| vcpkg | 0.2.15 | MIT/Apache-2.0 |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| want | 0.3.1 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.3+wasi-0.2.9 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip3 | 0.4.0+wasi-0.3.0-rc-2026-01-06 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.122 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.72 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.122 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.122 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.122 | MIT OR Apache-2.0 |
| wasm-encoder | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-metadata | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-streams | 0.4.2 | MIT OR Apache-2.0 |
| wasmparser | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| web-sys | 0.3.99 | MIT OR Apache-2.0 |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| webpki-roots | 1.0.7 | CDLA-Permissive-2.0 |
| winapi | 0.3.9 | MIT/Apache-2.0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| winapi-util | 0.1.11 | Unlicense OR MIT |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| windows | 0.62.2 | MIT OR Apache-2.0 |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 |
| windows-core | 0.62.2 | MIT OR Apache-2.0 |
| windows-future | 0.3.2 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.48.0 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.48.5 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.48.5 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.48.5 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.48.5 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| wit-bindgen | 0.51.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-bindgen-core | 0.51.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-bindgen-rust | 0.51.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-bindgen-rust-macro | 0.51.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-component | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-parser | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| writeable | 0.6.3 | Unicode-3.0 |
| yoke | 0.8.3 | Unicode-3.0 |
| yoke-derive | 0.8.2 | Unicode-3.0 |
| zerocopy | 0.8.50 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.50 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.7 | Unicode-3.0 |
| zeroize | 1.8.2 | Apache-2.0 OR MIT |
| zerotrie | 0.2.4 | Unicode-3.0 |
| zerovec | 0.11.6 | Unicode-3.0 |
| zerovec-derive | 0.11.3 | Unicode-3.0 |
| zmij | 1.0.21 | MIT |
