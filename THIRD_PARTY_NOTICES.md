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

## Exact runtime source review

`licenses/runtime-inventory.json` records the locked source archives, their
SHA-256 hashes and individual upstream/distributed notice hashes. Regenerate
with `python3 tools/runtime_notices.py` after realizing the recorded Nix source
paths. The Nix package already installs this entire tree under share/doc/drift.
Whitespace normalization preserves the upstream wording.

Rust 1.99.0: root MIT/Apache texts, COPYRIGHT and license-metadata.json, plus all
76 selected root/library notice files, are retained conservatively, including
vendor notices for unbuilt platforms. Compiler-builtins declares MIT AND
Apache-2.0 WITH LLVM-exception for the crate, rather than a simple MIT alternative;
its full license text is supplied. libm declares MIT. Rust's COPYRIGHT explicitly
points to generated COPYRIGHT-library.html for binary-release standard-library
attribution. That artifact-specific generated document is not present in the
installed toolchain/source archive reviewed here. Source notices improve coverage
but do not establish exact attribution completeness for the linked standard
library. Obtaining or reconstructing that binary-specific attribution remains
a publication blocker.

The current Nix executable dynamically references glibc (libc, libm and loader)
and GCC libgcc_s. Exact glibc 2.44 source provides COPYING.LESSERv2 (LGPL 2.1),
COPYINGv2, COPYINGv3 and LICENSES; all are supplied. LICENSES expressly requires
its additional third-party notices with binary distributions. This is more
specific than the coarse LGPL-2.0-or-later label in Nixpkgs package metadata.
GCC 16.2.0 root COPYING files, including GPLv3 and GCC Runtime Library Exception
3.1, are supplied. The exception applies to files bearing the relevant notice
and eligible compilation processes; package-level GPL metadata alone does not
capture those permissions. This is not a determination that every file in the
native output is covered by that exception.

Nix build tools are not automatically shipped with Drift merely because they
build it. Drift's output references native libraries instead of copying them.
Exporting a Nix closure redistributes those outputs separately, with their own
license and corresponding-source requirements. The source archive hashes here
identify upstream inputs, not a complete corresponding-source distribution of
Nixpkgs patches/build recipes for every exported library. No written source offer
is made by this document. Prepare and verify the source/notices mechanism for a
closure export before publication. That distribution step remains a blocker.

A future standalone archive must carry the project and third-party notice trees
alongside the executable and resolve its Nix store runtime paths or define a
separately reviewed portable build. No portable binary archive has been prepared
or cleared. Other target-specific Cargo sources and native closure components
have not received comprehensive review. These limits are publication blockers,
not an assertion of complete legal compliance.

Drift's copyright attribution is Copyright (c) 2026 Carlos Enrique Jimenez Sosa,
matching the owner-approved local Pulse LICENSE convention. The supplied public
Pulse URL was unavailable; the local checkout points to pulse-systemd.
