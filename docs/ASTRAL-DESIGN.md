# Astral 0.6 interface

The native interface uses a shared graphite palette, a restrained blue accent and original vector glyphs. The canvas and animation stay visually dominant. The direction draws on Apple's guidance for [grouping toolbar commands](https://developer.apple.com/design/human-interface-guidelines/toolbars) and [clearly communicating button actions](https://developer.apple.com/design/human-interface-guidelines/buttons). This is a Windows application with its own icons and native window controls.

## Color and hierarchy

| Role | Color |
| --- | --- |
| Background | `#131417` |
| App chrome | `#1B1C20` |
| Panels and sheets | `#222328` |
| Controls | `#2D2E35` |
| Canvas surround | `#27292F` |
| Accent | `#689DF5` |
| Selection | `#304A71` |
| Primary text | `#F0F1F5` |
| Secondary text | `#A5AAB8` |

Regular and bold Noto Sans remain bundled. Body text is 13 points, secondary text 11 points, and section headings use a consistent hierarchy. Controls use 9-point corners; sheets use 14-point corners. Borders and separators are quiet. Blue identifies selection and primary actions.

## Controls

The original line icons are drawn directly by egui. They require no platform icon font, bitmap scaling or network resources. Standard icon controls have a 32 × 32 logical-point hit area around a 16-point glyph. Tab-close buttons use a compact 26-point target. Inactive toolbar icons are flat; hover, keyboard focus and selection provide feedback. Every icon button supplies its action name to AccessKit and displays a tooltip, including relevant shortcuts.

The project library uses icons for open, image import, settings, favorites and project options. The editor uses icons for navigation, file/add/view menus, save, drawing tools, undo/redo, assistance and command search. Frame tools share one horizontally scrollable row. Layer visibility, locking, ordering and options use compact icon controls. Familiar action labels remain in menus, primary Create/Export actions, workspace names, property names and presets.

## Sizing

The drawing frame strip reserves space for its controls and previews. The pen canvas has a stable interaction ID, and showing completion controls does not shift its coordinates. Long project and layer names truncate within their allotted widths.

**Settings → General → Fit interface to window** is enabled by default. It preserves the requested scale as far as possible while keeping at least an 800 × 520 logical-point working area on supported window sizes. The saved scale preference is retained when a smaller window temporarily needs a lower applied scale. Disable the option to use the chosen scale without fitting. The settings panel displays the applied scale.

Settings, voice, assistance, recognition and export sheets size themselves to the logical viewport and scroll when necessary. New-project fields scroll independently of the Create/Cancel footer; Export keeps its action outside its scrolling settings. Versioned sheet IDs reset the old initial 400-point sheet sizes while retaining project data and preferences.

## Coverage

Headless egui/AccessKit regression checks cover accessible icon names, 32-point hit areas and onscreen editor controls at 800 × 520, 960 × 620 and 1280 × 800. Additional checks cover the new-project footer in short windows, scale fitting and unchanged pen/eraser/move/undo behavior. These checks complement live native UI review; they do not constitute a complete accessibility audit.

The document engine, animation commands, MCP tool contract and media formats retain their existing behavior. Projects continue using the existing schema; the UI adds a defaulted desktop preference for scale fitting.

## Alignment correction (0.6.1)

Project cards explicitly stack their thumbnail, title, metadata, path and action row in a vertical child layout. Grid gaps and content widths include borders and scrollbars. Long names truncate. The library footer stays outside the scrolling cards. Narrow inspector sliders move their captions above the controls, action rows wrap, and onion counts use a two-column grid. Fresh profiles start maximized so the editor footer is not hidden below the screen edge. Regression coverage includes populated libraries and minimum-width editor panels.
