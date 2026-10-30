# Anatomy model licence

`body.glb` is not covered by the MIT licence of the rest of this project.

It is derived from **BodyParts3D**, © The Database Center for Life Science, licensed under
[CC Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
(<https://dbarchive.biosciencedbc.jp/en/bodyparts3d/lic.html>). The source files themselves
state CC Attribution-Share Alike 2.1 Japan, so this derived model is released under
[CC Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/),
which satisfies both.

Changes made: 56 regions chosen from the PART-OF set (release 4.0), the parts of each merged,
converted from millimetres to metres with Y up, simplified, quantized and compressed with
meshoptimizer. `scripts/anatomy/build.mjs` rebuilds it from the original download.
