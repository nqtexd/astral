# Scene interactions — Astral 0.9.1

Use `scene_interactions` to inspect or replace a project's declared prop contacts and state requirements. Writes require `project` and `expected_revision`; stale revisions fail without changes. Replacing cues is one undoable transaction. The matching CLI/command operation is `set_scene_interactions` in a command batch. `review_animation` returns `interactions` alongside melee contacts, torso overlaps and storyboard warnings.

Each cue has `name`, inclusive `start`/`end` frames and one `kind`:

- `touch`: `character`, `joint`, `graphic`, `offset: {x,y}`, `tolerance`. The joint must remain within tolerance of the graphic's transformed local offset at every frame. Rotation, scale and live joint bindings are respected. A transfer needs a giver touch, a receiver touch, and a short overlap where both touch the same prop. Animate the prop and change its ownership/binding explicitly; this tool checks the result and never invents a handoff.
- `graphic_state`: `graphic`, `min_opacity`, `max_opacity`. Check a lamp/door/prop's animated visibility. Outside its active range the graphic has zero opacity.
- `passage`: `character`, `barrier`, `crossing_x`, `half_width`, `maximum_opacity`. The character's root must actually cross from one side of the threshold to the other. While inside the threshold band, the blocking graphic must be transparent/absent. For a sliding door, animate the blocking leaf opening, then use a separate parked leaf to keep the open door visible.

All interaction frames are checked even if `review_animation.step` is larger than one. A full passage range is needed to prove completion; a partial review only checks safety in the covered interval. A review accepts at most 32,768 interaction-frame checks; split longer work into shot ranges. Projects allow 256 cues of at most 4097 frames each. Missing rigs/joints/graphics, invalid ranges and non-finite parameters fail validation, including on project load. Removing a referenced prop requires removing/updating its cues in the same transaction.

Example arguments to `scene_interactions` (replace IDs with inspected IDs):

```json
{
  "project": "PROJECT_UUID",
  "expected_revision": 12,
  "cues": [
    {
      "name": "Press release",
      "start": 60, "end": 180,
      "kind": "touch",
      "character": "RIG_UUID", "joint": "right_hand",
      "graphic": "BUTTON_UUID", "offset": {"x": 0, "y": 0},
      "tolerance": 2
    },
    {
      "name": "Exit through open door",
      "start": 240, "end": 360,
      "kind": "passage", "character": "RIG_UUID",
      "barrier": "CLOSED_LEAF_UUID", "crossing_x": 780,
      "half_width": 25, "maximum_opacity": 0.01
    }
  ]
}
```

For stickman production: plan the storyboard and props first; block roots within reach; pose hands with `motion_tools` SmartPose/MotionBrush; use consistent elbow bends; bind hand graphics to forearms; align effect/audio markers; declare contacts and interactions; run `review_animation` per shot; inspect moving anticipation/contact/recovery previews; then export. A dialogue line such as “open it” is not evidence that anything opened. These declared geometric/state tests do not infer story meaning, validate finger anatomy or certify artistic quality.

Running arms now blend from their original poses, keep their elbow branch and follow the leaned torso. The After the Siren source adds bone-length-preserving angular blends between locomotion, guard and reaching. Native vector hand artwork rotates with forearms. No new model or generated-frame training is involved in these corrections.
