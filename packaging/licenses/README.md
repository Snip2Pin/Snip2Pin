# License collection sources

`about.toml` supplies checksum-bound cargo-about clarifications for license texts
omitted from published crates, or not recognized by automatic detection. Remote
files are fetched from the crate's published `.cargo_vcs_info.json` commit.

`reviewed.json` is the narrow exception list for upstream versions without a full
license text. Every entry binds the crate version, published source commit and
the published Cargo.toml checksum. Collection preserves the upstream license and
authors metadata and labels the standard SPDX text as a reference, rather than
presenting it as an upstream copyright notice. An unreviewed fallback fails.

The two objc2 files are verbatim upstream sources: `LICENSE.txt` from older
published objc2 commits and `LICENSE.md` from newer ones. The latter is an
upstream licensing statement with links, rather than a full MIT license. Each
reviewed entry binds the appropriate statement's SHA-256; no copyright holder is
inferred from package authors. `dispatch` and `drm-fourcc` provide a manifest
license declaration but no separate full license file at their published commits.

The embedded fonts are collected separately from their registry package. All
four actual font notice files are retained, including the Hack/Bitstream text,
and checksummed so a license change requires review.

`NSIS-3.13-COPYING.txt` is the official distribution's copyright and license
information, included under a Windows-installer-only heading. Its provenance is:

- Archive: <https://downloads.sourceforge.net/project/nsis/NSIS%203/3.13/nsis-3.13.zip>
- Archive SHA-256: `ba63dffc4410ee89193e1cb5a41989991bd77c61068da17e3156d136b7b0b3d8`
- Member: `nsis-3.13/COPYING`
- Member SHA-256: `388357c1215ff403c5ebde3a5ecd273e68f8b79a579996775245d1ee65442aba`
- Stored SHA-256 after LF and trailing-whitespace normalization: `9afab55b28917f993049218d52d5ed9ec56498bc6d53fe422aea5b60c867ee4b`
