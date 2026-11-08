# Third-party notices for Drift 0.1.0

Drift's MIT license applies to Drift's own code. Dependencies retain their own
licenses. `licenses/inventory.json` records the Linux Cargo graph and copies of
upstream root license/copyright/notice files, with SHA-256 hashes. License wording comes from the locked local crate sources; only line endings,
trailing whitespace and blank end lines are normalized. Source and distributed
file hashes are recorded. Regenerate with
`python3 tools/license_inventory.py` after dependency changes; review changes.

The inventory covers 59 packages for x86_64-unknown-linux-gnu, including direct,
transitive, development and build dependencies. `roles` describes reachability:
normal, build, or dev; `direct` identifies edges from Drift. These are Cargo
roles, not a binary-level determination of included code. Proc macros run during
compilation; normal runtime libraries are candidates for static inclusion, while
build-only tools and tempfile's development graph are not ordinarily shipped in
the executable. Compiler-generated code may cross these boundaries. All inventoried
notices are supplied conservatively, including those for build/dev packages.

The declared expressions include MIT, Apache-2.0, BSD-2-Clause alternatives,
Unlicense alternatives, LLVM exception alternatives, and Unicode-3.0. Historical
`MIT/Apache-2.0` strings are retained exactly as upstream metadata, not rewritten
as a new license determination. No ISC or MPL expression occurs in this Linux
Cargo inventory. For MIT alternatives, retain the upstream copyright and permission
notice; Apache texts and any upstream NOTICE material must be retained when that
license applies. Unicode-ident additionally declares Unicode-3.0; its Unicode
license text is included. Do not drop conjunctive obligations when selecting an
alternative license. No copyright holder names have been invented.

SQLite is compiled into the executable by libsqlite3-sys's bundled feature. Its
amalgamation header dedicates the SQLite code to the public domain; that exact
header is included as SQLITE-HEADER.txt. The wrapper's MIT license is also copied.
SQLCipher is not enabled and is not part of this build.

## Distribution formats and remaining review

The Nix output installs LICENSE, this document, and the inventory/license tree
under `share/doc/drift`. A future standalone archive must explicitly include
these files alongside its binary; repository presence alone is insufficient.
Do not distribute a bare executable and assume these notices accompany it.

The executable also incorporates Rust standard-library/compiler runtime code,
which this Cargo inventory does not enumerate. Rust runtime notices and any
additional bundled components require review before standalone distribution.

The current Nix executable dynamically references glibc (libc, libm and loader)
and GCC libgcc_s. The locked Nixpkgs metadata identifies glibc as LGPL-2.0-or-later and
GCC as GPL-3.0-or-later at package level. That coarse metadata does not
express the component-specific GCC runtime exception; exact output-component
terms, source availability and redistribution requirements must be verified
against the locked native sources before redistributing their closure. Nix build
tools (compiler, Cargo, pkg-config and build hooks) are not automatically included
in Drift's output merely because they build it. The Nix store output contains
references to native libraries rather than copies of those libraries; exporting
a closure redistributes those dependencies separately. This inventory is not a
complete license clearance for a Nix closure or standalone archive.

Other target-specific Cargo sources were not inventoried. No compliance claim is
made for unbuilt targets. The copyright holder of Drift is still a placeholder
in LICENSE and must be confirmed. These are publication blockers; the copied
Cargo notices alone do not establish complete compliance.
