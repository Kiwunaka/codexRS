# codexRS patch

This directory contains the source published as `gpui-component` 0.5.1 from
<https://crates.io/crates/gpui-component/0.5.1>. The upstream repository is
<https://github.com/longbridge/gpui-component>.

codexRS keeps this pinned local copy because the released native Markdown
renderer does not expose source-range highlights. The local change adds that
single API while preserving the upstream renderer, selection, links, code
blocks, and license. Remove the patch when an equivalent released upstream API
is available.
