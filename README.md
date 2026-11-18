# Evidence for the RAW cutover render check (storytold/craft-libs#587)

Images only. This branch is not meant to be merged.

All source files are CC0 samples from raw.pixls.us. They are listed in LightCraft's `xtask/src/raw_samples.rs`:

| Image | Source |
|---|---|
| `nikon-z7-coverage-2789-corners.jpg` | Nikon Z 7, https://raw.pixls.us/getfile.php/2789/nice/Nikon%20-%20Z%207%20-%2012bit%2012bit%20compressed%20%283:2%29.NEF |
| `nikon-z6-coverage-3581-corners.jpg` | Nikon Z 6, https://raw.pixls.us/getfile.php/3581/nice/Nikon%20-%20Z%206%20-%2014bit%2014bit%20compressed%20%283:2%29.NEF |
| `fuji-x-s1-cfa.jpg` | Fujifilm X-S1, https://raw.pixls.us/getfile.php/997/nice/Fujifilm%20-%20X-S1%20-%2012bit%2012bit%20uncompressed%20%284:3%29.RAF |
| `fuji-hs50exr-cfa.jpg` | Fujifilm FinePix HS50EXR, https://raw.pixls.us/getfile.php/6018/nice/Fujifilm%20-%20FinePix%20HS50EXR%20-%2012bit%2012bit%20uncompressed%20%284:3%29.RAF |
| `fuji-x-a2-control-cfa.jpg` | Fujifilm X-A2 (labels unchanged by the cutover), https://raw.pixls.us/getfile.php/2883/nice/Fujifilm%20-%20X-A2%20-%2012bit%2012bit%20uncompressed%20%283:2%29.RAF |

## Nikon `*-corners.jpg`

The four corners of the frame (one row each), each shown in three columns:
1. the camera's embedded JPEG;
2. `lightcraft-cli render --size 1200` at `989735d60` (before the craft-raw cutover);
3. the same render at `27b755a93` (main after the cutover).

Before the cutover the render shows a wider field of view than the camera JPEG. After it, the framing matches.

## Fuji `*-cfa.jpg`

Three columns:
1. the camera's embedded JPEG;
2. a naive debayer of the raw data with the CFA labels craft-raw now reports;
3. the same debayer with red and blue swapped, which gives the labels LightCraft's decoder reported before the cutover (for the X-A2 it is only a swap test).

The debayer applies only the as-shot white balance multipliers and a 2.2 gamma. It applies no colour matrix, so its colours are flat. Judge hue, not saturation: the X-S1's red shed and the HS50EXR's blue sky come out right only in column 2.
