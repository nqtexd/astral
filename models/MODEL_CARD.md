# Astral masked pose prior v1

Purpose: fill missing joints of the canonical eleven-joint 2D Astral stick rig. This model outputs editable vector joint positions. It is not an image/video generator, image recognizer, language model or temporal motion model.

Architecture: 33 inputs (22 rest-relative normalized coordinates, 11 observation masks), 64 tanh hidden units, 22 outputs. Total 3,606 trainable parameters. Visible joints are restored exactly after prediction; world-space completion runs bone-length constraints while pinning observations. At least two observed controls are required.

Data: 8,192 procedurally generated masked poses from Astral's eight action presets, with varied action phase, intensity, proportions, translation and rotation. No external images, personal data or third-party dataset. Training code and weights are distributed under the repository GPL license. Random seed 71826152767820. Adam, minibatches of 64, learning rate 0.001, 100 epochs. The held-out split uses 1,024 poses from disjoint trajectory seeds; this remains synthetic data from the same generator family.

Recorded training run: 4.814 seconds on the available development computer. Held-out missing-coordinate MSE 0.00969554 versus 0.06569623 for unchanged rest pose; relative reduction 85.24%. Observed-joint drift zero by construction. Values describe raw normalized network output, not end-to-end artwork quality or a comparison with linear interpolation. Read `pose-prior-v1-report.json` for the exact recorded metrics. A second test evaluates fresh held-out seeds. There is no validated real-world accuracy claim.

Reproduce from the repository:

```powershell
cargo run --release -p astral-assist --example train
cargo test -p astral-assist
```

The trainer writes `pose-prior-v1.json` and its evaluation report in this directory. CPU execution only, no external runtime or API key. The embedded default weights are compiled into Astral; exported models can be loaded from the project's `models/` folder. The UI training job trains the same synthetic prior, not the user's imported artwork. Drawing examples train a separate RBF trajectory model.

Limitations: canonical control order/topology only; plausible training-distribution stick poses only; no contact reasoning, semantic action generation, occlusion understanding or preservation of arbitrary silhouettes. Limb constraints can alter raw predictions. Review and adjust suggestions. Raster stickman mapping is a separate heuristic segmentation/skeleton tool with manual landmark correction. Keep authored keyframes and use undo when a suggestion does not fit.
