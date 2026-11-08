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

`licenses/runtime-inventory.json` records the locked Nix source archives,
Rust's matching official release archive, SHA-256 hashes, and individual
upstream/distributed notice hashes. Regenerate by realizing each `nix_source`
path in the inventory with `nix-store --realise PATH`, then running
`python3 tools/runtime_notices.py`. The script verifies each source archive
hash and the Rust channel manifest and archive hashes, then
copies `COPYRIGHT-library.html` byte-for-byte. Other source notice text only
has line endings, trailing whitespace, and blank end lines normalized. The Nix
package installs this entire tree under `share/doc/drift`.

Rust 1.99.0: root MIT/Apache texts, COPYRIGHT and license-metadata.json, plus all
76 selected root/library notice files, are retained conservatively, including
vendor notices for unbuilt platforms. Compiler-builtins declares MIT AND
Apache-2.0 WITH LLVM-exception for the crate, rather than a simple MIT alternative;
its full license text is supplied. libm declares MIT. Rust's COPYRIGHT explicitly
points to generated `COPYRIGHT-library.html` for binary-release standard-library
attribution. The exact Rust compiler used by the locked Nix build is
`rustc 1.99.0 (b940084d7eb6a299eb4bfeb8e34901bc051e7ac4)`, LLVM 21.1.8. The
same compiler commit is embedded in the official Rust 1.99.0 x86_64 Linux
release artifact. Its channel manifest, archive, path within the archive, and
the byte-for-byte document hash are recorded in `licenses/runtime-inventory.json`.
This generated notice covers the Rust standard-library code linked by Drift;
the broader source notices remain included as well.

The x86_64 Linux executable directly needs `libgcc_s.so.1`, `libm.so.6`,
`libc.so.6`, and `ld-linux-x86-64.so.2`. Its Nix runtime closure also contains
glibc 2.44, GCC 16.2.0 runtime outputs, libidn2 2.3.8, and libunistring 1.4.2.
The last two are in the glibc closure; they are not direct ELF `NEEDED`
entries. Nixpkgs metadata declares glibc LGPL-2.0-or-later, GCC runtime
GPL-3.0-or-later, and libunistring LGPL-3.0-or-later. The linked libidn2
library is LGPL-3.0-or-later; Nixpkgs also lists GPL terms for other libidn2
package components. The exact glibc 2.44 source provides COPYING.LESSERv2
(LGPL 2.1), COPYINGv2, COPYINGv3 and LICENSES; all are supplied. LICENSES
requires additional third-party notices with binary distributions. GCC 16.2.0
COPYING files, including GCC Runtime Library Exception 3.1, are supplied. The
exception applies only where its stated notice and compilation conditions hold.
The exact libidn2 and libunistring source notices are included too.

The source archive hashes identify exact upstream inputs. They are provenance,
not by themselves corresponding-source distribution. For any future Nix
closure distribution, first inventory the actual output and closure with
`nix path-info --recursive ./result`; for each non-Drift path inspect its
deriver using `nix-store --query --deriver PATH`, then inspect derivation inputs
with `nix derivation show DERIVATION`. Prepare a matching Nix binary cache with
both `nix copy --to file:///DESTINATION --recursive ./result` and
`nix copy --to file:///DESTINATION --derivation --recursive ./result`; the
second copies derivations and their input closure. Verify the destination
contains every output, derivation, source archive and patch input required by
the closure. Include the locked nixpkgs revision, this notice tree, and all
package notices. This repository does not currently export that cache or make
a written source offer. Do not describe a closure export as ready until the
content is verified and recipients have a documented source-availability path.

The v0.1.0 publication scope is this source repository and instructions for
users to build locally with Nix. Drift does not publish a prebuilt package or
runtime closure. A local `nix build` obtains/builds dependencies through the
user's configured Nix store and substituters; Drift does not redistribute
those paths. Native closure redistribution is therefore not a blocker for
this source-only scope. If Drift later hosts or transfers a package/closure,
the source-and-notice procedure above becomes a release gate. The Nix output
still installs this notice tree alongside the locally built executable.

No standalone binary archive is in v0.1.0 scope or has been prepared. Any
future standalone archive needs a separate closure and corresponding-source
review and must carry the project and third-party notice trees. Other
target-specific Cargo sources and native closure components have not received
comprehensive review. This is not a claim of universal legal compliance.

Drift's verified copyright attribution is Copyright (c) 2026 Carlos Enrique
Jimenez Sosa.
