use crate::{
    ipc,
    service::{Request, Service},
};
use astral_core::{
    Batch, Color, Command, Dialogue, DialogueAppearance, DialogueStyle, Easing, Graphic,
    GraphicKey, GraphicKind, GraphicLayer, Id, InkStroke, Point,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::{server::router::tool::ToolRouter, server::wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone)]
pub struct AstralMcp {
    service: Service,
    live: bool,
    tool_router: ToolRouter<Self>,
}
#[derive(Deserialize, JsonSchema)]
pub struct LearnMotionArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub character: Id,
    pub smoothing: Option<f64>,
}
#[derive(Deserialize, JsonSchema)]
pub struct LearnDrawingArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub sources: Vec<Id>,
    pub smoothing: Option<f64>,
}
#[derive(Deserialize, JsonSchema)]
pub struct DialogueArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub text: String,
    pub speaker: Option<String>,
    pub start: u32,
    pub end: u32,
    pub character: Option<Id>,
    pub position: Option<Point>,
    pub style: Option<DialogueStyle>,
    pub typewriter: Option<f32>,
    pub formatting: Option<astral_core::TextFormatting>,
    pub font_size: Option<f32>,
    pub appearance: Option<DialogueAppearance>,
    pub color: Option<Color>,
    pub text_color: Option<Color>,
    pub size: Option<Point>,
}
#[derive(Deserialize, JsonSchema)]
pub struct GraphicArgs {
    /// Optional local vector contours for custom silhouettes, props and artwork.
    pub contours: Option<Vec<Vec<Point>>>,
    pub project: Id,
    pub expected_revision: u64,
    pub kind: GraphicKind,
    pub start: u32,
    pub end: u32,
    pub position: Point,
    pub end_position: Option<Point>,
    pub size: Point,
    pub color: Option<[u8; 4]>,
    pub text: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
pub struct ProjectArgs {
    pub project: Id,
}
#[derive(Deserialize, JsonSchema)]
pub struct InspectArgs {
    pub project: Id,
    pub frame: Option<f64>,
}
#[derive(Deserialize, JsonSchema)]
pub struct OpenArgs {
    pub path: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct SaveArgs {
    pub project: Id,
    pub path: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct PreviewArgs {
    pub project: Id,
    pub frame: f64,
    pub width: u32,
    pub height: u32,
    pub path: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
pub struct CreateArgs {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: u32,
}
#[derive(Deserialize, JsonSchema)]
pub struct HistoryArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub redo: bool,
}
#[derive(Deserialize, JsonSchema)]
pub struct JobArgs {
    pub job: Id,
}
#[derive(Deserialize, JsonSchema)]
pub struct AssembleArgs {
    pub project: Id,
    pub spec: crate::export::AssemblySpec,
}
#[derive(Deserialize, JsonSchema)]
pub struct ExportArgs {
    pub project: Id,
    pub spec: crate::export::ExportSpec,
}
#[derive(Deserialize, JsonSchema)]
pub struct AssetArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub path: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Default, PartialEq, clap::ValueEnum)]
pub enum Profile {
    #[default]
    All,
    Projects,
    Animation,
}
#[derive(Deserialize, JsonSchema)]
pub struct ManagedArgs {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: u32,
    pub path: String,
    pub background: Option<[u8; 4]>,
}
#[derive(Deserialize, JsonSchema)]
pub struct CloseArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub discard: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
pub struct FigureArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub name: String,
    pub position: Point,
    pub color: Option<[u8; 4]>,
}
#[derive(Deserialize, JsonSchema)]
pub struct AnimateArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub character: Id,
    pub action: astral_core::ActionKind,
    pub start: u32,
    pub end: u32,
    pub intensity: Option<f32>,
    pub distance: Option<f32>,
    pub direction: Option<f32>,
}
#[derive(Deserialize, JsonSchema)]
pub struct FrameArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub layer: Id,
    pub frame: u32,
    pub strokes: Vec<astral_core::InkStroke>,
}
#[derive(Deserialize, JsonSchema)]
pub struct LayerArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub name: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct ImageArgs {
    pub project: Id,
    pub path: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct RigArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub path: String,
    pub detection: astral_assist::recognition::Detection,
    pub name: String,
    pub center: Point,
    pub height: f32,
    pub frame: u32,
}
#[derive(Deserialize, JsonSchema)]
pub struct PoseCompleteArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub character: Id,
    pub frame: u32,
    pub observed: Vec<String>,
    pub model_path: Option<String>,
    pub apply: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
pub struct CelCompleteArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub layer: Id,
    pub sources: Vec<u32>,
    pub frame: u32,
    pub smoothing: Option<f64>,
    pub apply: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
pub struct CelGenerateArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub layer: Id,
    pub sources: Vec<u32>,
    pub start: u32,
    pub end: u32,
    pub smoothing: Option<f64>,
    pub overwrite: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
pub struct SuggestArgs {
    pub project: Id,
    pub layer: Id,
    pub frame: u32,
    pub strength: Option<f32>,
}
#[derive(Deserialize, JsonSchema)]
pub struct TrainArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub samples: Option<usize>,
    pub epochs: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
pub struct VoiceArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub text: String,
    pub start: u32,
    pub character: Option<Id>,
    pub options: Option<crate::voice::VoiceOptions>,
    pub dialogue: Option<bool>,
    pub formatting: Option<astral_core::TextFormatting>,
    pub speaker: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct StoryboardArgs {
    pub project: Id,
    pub expected_revision: u64,
    /// Omit to inspect. Replace the entire validated board in one undoable revision.
    pub board: Option<astral_core::storyboard::Storyboard>,
}
#[derive(Deserialize, JsonSchema)]
pub struct ReviewArgs {
    pub project: Id,
    pub start: u32,
    pub end: u32,
    /// Sampling stride in frames. Declared contact frames are always included.
    pub step: u32,
}
#[derive(Deserialize, JsonSchema)]
pub struct InteractionArgs {
    pub project: Id,
    pub expected_revision: u64,
    /// Omit to inspect. Replacing cues is one validated, undoable transaction.
    pub cues: Option<Vec<astral_core::interactions::InteractionCue>>,
}
#[derive(Deserialize, JsonSchema)]
pub struct AutoAnimateArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub spec: astral_core::director::AutoSpec,
    #[serde(default)]
    pub apply: bool,
}
#[derive(Deserialize, JsonSchema)]
pub struct DrawFrameArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub layer: Id,
    /// Zero-based drawing frame. No other frame receives this ink.
    pub frame: u32,
    pub strokes: Vec<InkStroke>,
}
#[derive(Deserialize, JsonSchema)]
pub struct RenameArgs {
    pub project: Id,
    pub expected_revision: u64,
    pub name: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct TrashArgs {
    pub project: Id,
    pub expected_revision: u64,
}
#[tool_router]
impl AstralMcp {
    #[tool(
        description = "Inspect linked media, relink missing audio/source files without changing stable IDs, create verified .astralpack snapshots, or unpack into a new workspace folder. Relink requires current revision; package/unpack return cancellable job IDs. All paths are confined to configured project/workspace roots. Embedded raster pixels remain unchanged by source-file relinking; re-import to replace artwork."
    )]
    async fn asset_tools(
        &self,
        Parameters(request): Parameters<crate::assets::AssetRequest>,
    ) -> CallToolResult {
        self.dispatch(Request::AssetTools { request }).await
    }
    #[tool(
        description = "Import bounded image sequences or layered 8-bit RGB PSD; paint independent bitmap cels; generate editable audio-envelope/text-guided mouth cues; convert short video clips to image sequences; track distinctive image patches into reviewed camera or graphic/raster object keys (source-pixel seed, at most 120 frames, optional second_point for rotation/scale and target object ID). Import audio normalizes bounded WAV/AIFF/MP3/M4A to editable full-length clips. Mutations require the current revision. Import/video return cancellable jobs. Paint points/radius are source pixels. PSD supports native group/mask/supported-blend graphs and basic editable text; font metrics, adjustment/filter/clipping semantics still require review. lip_sync engine english/phonetic uses offline recognition with configurable viseme mapping; envelope is approximate. Use apply_commands PutRasterLayer for transforms, translation/MLS-similarity pins, cubic curve keys and bone-angle corrective drivers."
    )]
    async fn artwork_tools(
        &self,
        Parameters(request): Parameters<crate::artwork::ArtworkRequest>,
    ) -> CallToolResult {
        self.dispatch(Request::Artwork { request }).await
    }

    #[tool(
        description = "Inspect the last 200 local project edits, changed document sections, revision numbers and timestamps. This is an activity log, not a complete version backup."
    )]
    async fn project_activity(&self, Parameters(a): Parameters<ProjectArgs>) -> CallToolResult {
        self.dispatch(Request::ProjectActivity { project: a.project })
            .await
    }
    #[tool(
        description = "Rename an open project's display title as one undoable transaction. Requires the current revision. Save afterward to persist; filenames and media paths remain stable."
    )]
    async fn rename_project(&self, Parameters(a): Parameters<RenameArgs>) -> CallToolResult {
        self.dispatch(Request::RenameProject {
            project: a.project,
            expected_revision: a.expected_revision,
            name: a.name,
        })
        .await
    }
    #[tool(
        description = "Save current edits and move the saved project file into its local Astral Trash. Closes the document and keeps linked assets. Requires the current revision; recoverable, not permanent deletion."
    )]
    async fn trash_project(&self, Parameters(a): Parameters<TrashArgs>) -> CallToolResult {
        self.dispatch(Request::TrashProject {
            project: a.project,
            expected_revision: a.expected_revision,
        })
        .await
    }
    #[tool(
        description = "Restore an Astral Trash project at a relative configured-workspace path. Refuses to overwrite an existing project or an open copy."
    )]
    async fn restore_project(&self, Parameters(a): Parameters<OpenArgs>) -> CallToolResult {
        self.dispatch(Request::RestoreProject { path: a.path })
            .await
    }

    #[tool(
        description = "Replace one zero-based drawing frame only. Preserves earlier/later drawings, existing authored next keys and legacy holds; adds a blank/restoration boundary after this frame. Use this for manual frame-by-frame animation. SetCel through apply_commands deliberately holds until the next key. Revision-checked, one undo step."
    )]
    async fn draw_frame(&self, Parameters(a): Parameters<DrawFrameArgs>) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::DrawFrame {
                project: a.project,
                expected_revision: a.expected_revision,
                layer: a.layer,
                frame: a.frame,
                strokes: a.strokes,
            },
        })
        .await
    }
    #[tool(
        description = "Bounded offline animation controller: create a seeded original two-stickman spar with editable storyboard, choreography, camera and impacts; inspect existing animation; improve rest-length errors; or continue selected rigs into an empty range for up to three seconds. Does not contain an LLM or generate audio. Omit seed to avoid recent move patterns. Max three repair rounds; ambiguous skids/timing remain suggestions. Defaults to a non-mutating plan with diagnostics and geometric quality gate. apply=true commits as one undoable revision only if contact/spacing/interaction checks pass. Review a moving preview, then save/export with existing tools; never interpret the gate as an artistic score."
    )]
    async fn auto_animate(&self, Parameters(a): Parameters<AutoAnimateArgs>) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::AutoAnimate {
                project: a.project,
                expected_revision: a.expected_revision,
                spec: a.spec,
                apply: a.apply,
            },
        })
        .await
    }
    #[tool(
        description = "Declare and inspect verifiable scene interactions: hand-to-prop touches (including handoffs), graphic visibility states and doorway crossings. Each cue has an inclusive frame range. Passage requires the character to cross the threshold while its blocking graphic is transparent; animate a separate open leaf if needed. Touch offset is graphic-local, including scale/rotation and live bindings. Writes are revision-checked and undoable. Use review_animation to test every interaction frame, then inspect a moving preview. Dialogue alone never proves an action happened."
    )]
    async fn scene_interactions(
        &self,
        Parameters(a): Parameters<InteractionArgs>,
    ) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::Interactions {
                project: a.project,
                expected_revision: a.expected_revision,
                cues: a.cues,
            },
        })
        .await
    }
    #[tool(
        description = "Plan before animating: inspect or replace the persistent storyboard, cast bible, rig assignments, ordered shot ranges, location, camera/audio intent, entry/exit character state and transition explanations. Returns continuity warnings. Writes use expected_revision and one undoable transaction. Does not generate motion. Plan explicit contact cues with set_contact_cues; use review_animation and moving previews before export."
    )]
    async fn storyboard(&self, Parameters(a): Parameters<StoryboardArgs>) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::Storyboard {
                project: a.project,
                expected_revision: a.expected_revision,
                board: a.board,
            },
        })
        .await
    }
    #[tool(
        description = "Read-only scene quality review: checks dialogue/head/body/foot/prop bounds collisions and off-camera dialogue, exact melee contacts, torso spacing, storyboard continuity and scene_interactions (prop touches, graphic states, completed doorway crossings). Every declared interaction frame is checked regardless of sample step. Reports failures without mutation. Review short shots and moving anticipation/contact/recovery previews before full export. Undeclared actions cannot be inferred from dialogue; checks do not guarantee artistic quality or physical simulation."
    )]
    async fn review_animation(&self, Parameters(a): Parameters<ReviewArgs>) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::Review {
                project: a.project,
                start: a.start,
                end: a.end,
                step: a.step,
            },
        })
        .await
    }

    #[tool(
        description = "Plan or apply intelligent animation edits: flow_motion through authored pose keys, layout_dialogue placement proposals, constrained pose assistance, editable fight choreography, motion-brush paths, impact packs, coaching diagnostics, ghost trajectories, timing/FPS, multi-frame operations, reusable characters/motions/symbols/combos, rig templates, camera direction and audio editing. Edit defaults to a non-mutating proposal; review plan.commands and preview image, then commit through apply_commands at the returned revision. Coaching is heuristic, not a quality verdict. Library file paths stay inside the project workspace."
    )]
    async fn motion_tools(
        &self,
        Parameters(request): Parameters<crate::production::ProductionRequest>,
    ) -> CallToolResult {
        self.dispatch(Request::Production { request }).await
    }

    #[tool(
        description = "Train offline RBF kernel-ridge motion from 3–64 authored key poses. Fills inbetweens without adding thousands of keys. Sources remain editable; editing a source invalidates the model. Bake using bake_motion via apply_commands."
    )]
    async fn learn_motion(&self, Parameters(a): Parameters<LearnMotionArgs>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::TrainMotion {
                    character: a.character,
                    name: "Learned motion".into(),
                    smoothing: a.smoothing.unwrap_or(0.0001),
                }],
            },
        })
        .await
    }
    #[tool(
        description = "Learn inbetweens from 3–64 corresponding vector strokes at distinct frames. Supply source drawing IDs. Arc-length alignment precedes supervised regression. Bake to individually editable drawings using bake_drawing. This does not generate raster images."
    )]
    async fn learn_drawing(&self, Parameters(a): Parameters<LearnDrawingArgs>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::TrainDrawing {
                    sources: a.sources,
                    name: "Learned drawing".into(),
                    smoothing: a.smoothing.unwrap_or(0.0001),
                }],
            },
        })
        .await
    }
    #[tool(
        description = "Add an editable speech bubble, thought bubble or caption with vector text and optional typewriter reveal. Position is in world coordinates, or an offset from the attached character root. Text appears in exported animation."
    )]
    async fn add_dialogue(&self, Parameters(a): Parameters<DialogueArgs>) -> CallToolResult {
        let dialogue = Dialogue {
            id: Id::new_v4(),
            name: "Dialogue".into(),
            text: a.text,
            speaker: a.speaker.unwrap_or_default(),
            start: a.start,
            end: a.end,
            position: a.position.unwrap_or(if a.character.is_some() {
                Point::new(-230., -340.)
            } else {
                Point::new(300., 120.)
            }),
            size: a.size.unwrap_or(Point::new(540., 190.)),
            font_size: a.font_size.unwrap_or(30.),
            color: a.color.unwrap_or([250, 248, 255, 255]),
            text_color: a.text_color.unwrap_or([32, 28, 48, 255]),
            appearance: a.appearance.unwrap_or_default(),
            style: a.style.unwrap_or_default(),
            character: a.character,
            typewriter: a.typewriter.unwrap_or(30.),
            formatting: a.formatting.unwrap_or_default(),
        };
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::AddDialogue { dialogue }],
            },
        })
        .await
    }
    #[tool(
        description = "Add a vector cloud, shape, ring, star, burst, arrow or lower third. Optional end_position animates travel. Use set_graphic_key/replace_graphic for scale, rotation, opacity, Bezier curves, foreground/background placement and draw-on."
    )]
    async fn add_motion_graphic(&self, Parameters(a): Parameters<GraphicArgs>) -> CallToolResult {
        let mut keys = vec![GraphicKey {
            frame: a.start,
            position: a.position,
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Smooth,
        }];
        if a.end > a.start {
            let mut last = keys[0].clone();
            last.frame = a.end;
            last.position = a.end_position.unwrap_or(a.position);
            keys.push(last)
        }
        let graphic = Graphic {
            contours: a.contours.unwrap_or_default(),
            id: Id::new_v4(),
            name: format!("{:?}", a.kind),
            kind: a.kind,
            start: a.start,
            end: a.end,
            size: a.size,
            color: a.color.unwrap_or([190, 170, 245, 255]),
            layer: if a.kind == GraphicKind::Cloud {
                GraphicLayer::Background
            } else {
                GraphicLayer::Foreground
            },
            text: a.text.unwrap_or_default(),
            keys,
            draw_on: false,
        };
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::AddGraphic { graphic }],
            },
        })
        .await
    }

    #[tool(
        description = "Create and save a managed project with its first drawing layer and blank frame. Path is a new .astral file relative to the configured workspace; subsequent asset/export paths use this project folder."
    )]
    async fn create_managed_project(
        &self,
        Parameters(a): Parameters<ManagedArgs>,
    ) -> CallToolResult {
        self.dispatch(Request::CreateManagedProject {
            name: a.name,
            width: a.width,
            height: a.height,
            fps: a.fps,
            duration: a.duration,
            path: a.path,
            background: a.background,
        })
        .await
    }
    #[tool(
        description = "List saved .astral files under the configured workspace, bounded to 512 results and three directory levels."
    )]
    async fn list_saved_projects(&self) -> CallToolResult {
        self.dispatch(Request::ListSavedProjects).await
    }
    #[tool(
        description = "Close an open project at the expected revision. Unsaved edits are rejected unless discard=true is explicitly provided."
    )]
    async fn close_project(&self, Parameters(a): Parameters<CloseArgs>) -> CallToolResult {
        self.dispatch(Request::CloseProject {
            project: a.project,
            expected_revision: a.expected_revision,
            discard: a.discard.unwrap_or(false),
        })
        .await
    }
    #[tool(
        description = "Create an editable stick character with canonical named controls. Position is in project coordinates."
    )]
    async fn create_character(&self, Parameters(a): Parameters<FigureArgs>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::AddCharacter {
                    name: a.name,
                    position: a.position,
                    color: a.color.unwrap_or([35, 38, 44, 255]),
                }],
            },
        })
        .await
    }
    #[tool(
        description = "Apply an editable action clip. Walk/run omitted distance uses a natural pace derived from rig size, duration and FPS; explicit distance overrides it. Actions: idle, walk, run, jump, turn, strike, recoil, fall. Timing uses inclusive source frames; keep clips on one character non-overlapping."
    )]
    async fn animate_character(&self, Parameters(a): Parameters<AnimateArgs>) -> CallToolResult {
        self.dispatch(Request::Production {
            request: crate::production::ProductionRequest::Animate {
                project: a.project,
                expected_revision: a.expected_revision,
                character: a.character,
                action: a.action,
                start: a.start,
                end: a.end,
                intensity: a.intensity.unwrap_or(1.),
                distance: a.distance,
                direction: a.direction.unwrap_or(1.),
            },
        })
        .await
    }
    #[tool(
        description = "Add an editable drawing layer. Returns its ID for set_drawing_frame and frame generation."
    )]
    async fn add_drawing_layer(&self, Parameters(a): Parameters<LayerArgs>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::AddLayer {
                    layer: astral_core::DrawingLayer::new(a.name),
                }],
            },
        })
        .await
    }
    #[tool(
        description = "Set one drawing key's stroke list. Empty strokes create a blank; drawing holds to the next key. To edit a held frame independently, copy its strokes to this frame first."
    )]
    async fn set_drawing_frame(&self, Parameters(a): Parameters<FrameArgs>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands {
            batch: Batch {
                project: a.project,
                expected_revision: a.expected_revision,
                commands: vec![Command::SetCel {
                    layer: a.layer,
                    frame: a.frame,
                    strokes: a.strokes,
                }],
            },
        })
        .await
    }
    #[tool(
        description = "Analyze a workspace PNG/JPEG containing one clear line stickman. Returns estimated named landmarks, observed masks, scores and warnings. Review/correct landmarks before import_stickman_rig; this is topology mapping, not general image understanding."
    )]
    async fn recognize_stickman(&self, Parameters(a): Parameters<ImageArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::Recognize {
                project: a.project,
                path: a.path,
            },
        })
        .await
    }
    #[tool(
        description = "Import reviewed image landmarks as a canonical editable rig, retaining the original image as a reference asset. Does not preserve arbitrary painted texture."
    )]
    async fn import_stickman_rig(&self, Parameters(a): Parameters<RigArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::ImportRig {
                project: a.project,
                expected_revision: a.expected_revision,
                path: a.path,
                detection: a.detection,
                name: a.name,
                center: a.center,
                height: a.height,
                frame: a.frame,
            },
        })
        .await
    }
    #[tool(
        description = "Infer missing joints with the bundled small trained pose MLP or a registered local model. Observed named joints are pinned. apply=false returns a proposal and commands without editing; synthetic training does not guarantee arbitrary-pose accuracy."
    )]
    async fn complete_pose(&self, Parameters(a): Parameters<PoseCompleteArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::CompletePose {
                project: a.project,
                expected_revision: a.expected_revision,
                character: a.character,
                frame: a.frame,
                observed: a.observed,
                model_path: a.model_path,
                apply: a.apply.unwrap_or(false),
            },
        })
        .await
    }
    #[tool(
        description = "Learn corresponding stroke trajectories from 3–64 keyed cel examples and propose missing strokes at a frame between them. Existing strokes are preserved. apply=false returns preview commands."
    )]
    async fn complete_frame(&self, Parameters(a): Parameters<CelCompleteArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::CompleteFrame {
                project: a.project,
                expected_revision: a.expected_revision,
                layer: a.layer,
                sources: a.sources,
                frame: a.frame,
                smoothing: a.smoothing.unwrap_or(0.0001),
                apply: a.apply.unwrap_or(false),
            },
        })
        .await
    }
    #[tool(
        description = "Train local RBF stroke trajectories from 3–64 corresponding cel keys and generate editable inbetween frames as one undoable transaction. Source keys are preserved, occupied keys are skipped unless overwrite=true. Returns a cancellable job."
    )]
    async fn generate_inbetweens(
        &self,
        Parameters(a): Parameters<CelGenerateArgs>,
    ) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::GenerateFrames {
                project: a.project,
                expected_revision: a.expected_revision,
                layer: a.layer,
                sources: a.sources,
                start: a.start,
                end: a.end,
                smoothing: a.smoothing.unwrap_or(0.0001),
                overwrite: a.overwrite.unwrap_or(false),
            },
        })
        .await
    }
    #[tool(
        description = "Propose reversible geometric line/curve completion for the last stroke. Returns ghost strokes and commands; does not mutate the drawing. This helper is geometric rather than neural."
    )]
    async fn suggest_drawing(&self, Parameters(a): Parameters<SuggestArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::SuggestStroke {
                project: a.project,
                layer: a.layer,
                frame: a.frame,
                strength: a.strength.unwrap_or(0.5),
            },
        })
        .await
    }
    #[tool(
        description = "Inspect the bundled pose-completion model, training provenance and held-out evaluation report."
    )]
    async fn model_info(&self) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::ModelInfo,
        })
        .await
    }
    #[tool(
        description = "Train a small pose-completion MLP on reproducible synthetic Astral action data, save the weights as a project asset and return evaluation metrics. This is distinct from learning your drawing examples. Returns a cancellable job."
    )]
    async fn train_pose_model(&self, Parameters(a): Parameters<TrainArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::TrainModel {
                project: a.project,
                expected_revision: a.expected_revision,
                samples: a.samples.unwrap_or(8192),
                epochs: a.epochs.unwrap_or(100),
            },
        })
        .await
    }
    #[tool(
        description = "Enumerate locally installed Windows text-to-speech voices. No network generation service is used."
    )]
    async fn list_voices(&self) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::ListVoices,
        })
        .await
    }
    #[tool(
        description = "Generate editable WAV speech and optional dialogue using the saved Voice settings. options.provider selects offline, elevenlabs or compatible; omit to use the configured provider. External services require explicit opt-in and local credentials from Settings or environment; they may charge credits. Never pass API keys in tool calls or projects. list_voices lists installed Windows voices only. External pitch must be 1.0. Returns a cancellable revision-checked job."
    )]
    async fn add_voice(&self, Parameters(a): Parameters<VoiceArgs>) -> CallToolResult {
        self.dispatch(Request::Assist {
            request: crate::assistance::AssistRequest::Voice {
                project: a.project,
                expected_revision: a.expected_revision,
                text: a.text,
                start: a.start,
                character: a.character,
                options: a.options.unwrap_or_default(),
                dialogue: a.dialogue.unwrap_or(true),
                formatting: a.formatting.unwrap_or_default(),
                speaker: a.speaker.unwrap_or_default(),
            },
        })
        .await
    }
    pub fn new(service: Service, live: bool) -> Self {
        Self::with_profile(service, live, Profile::All)
    }
    pub fn with_profile(service: Service, live: bool, profile: Profile) -> Self {
        let mut tool_router = Self::tool_router();
        let project_tools = [
            "project_activity",
            "rename_project",
            "trash_project",
            "restore_project",
            "storyboard",
            "list_projects",
            "create_project",
            "create_managed_project",
            "open_project",
            "save_project",
            "inspect_project",
            "recover_project",
            "close_project",
            "list_saved_projects",
            "history",
        ];
        for tool in tool_router.list_all() {
            let name = tool.name.as_ref();
            let keep = match profile {
                Profile::All => true,
                Profile::Projects => project_tools.contains(&name),
                Profile::Animation => ![
                    "create_project",
                    "create_managed_project",
                    "close_project",
                    "list_saved_projects",
                ]
                .contains(&name),
            };
            if !keep {
                tool_router.remove_route(name);
            }
        }
        Self {
            service,
            live,
            tool_router,
        }
    }
    async fn dispatch(&self, request: Request) -> CallToolResult {
        let result = if self.live {
            ipc::call(self.service.workspace.root(), request).await
        } else {
            let service = self.service.clone();
            match tokio::task::spawn_blocking(move || service.request(request)).await {
                Ok(v) => v,
                Err(e) => Err(e.into()),
            }
        };
        match result {
            Ok(mut value) => {
                let mut content = vec![];
                for key in ["original_image_base64", "image_base64"] {
                    if let Some(image) = value
                        .as_object_mut()
                        .and_then(|v| v.remove(key))
                        .and_then(|v| v.as_str().map(str::to_owned))
                    {
                        content.push(ContentBlock::text(if key == "original_image_base64" {
                            "Original"
                        } else {
                            "Proposed"
                        }));
                        content.push(ContentBlock::image(image, "image/png"));
                    }
                }
                content.push(ContentBlock::text(value.to_string()));
                let mut result = CallToolResult::success(content);
                result.structured_content = Some(value);
                result
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error:#}"))]),
        }
    }
    #[tool(
        description = "List open Astral projects and their current revisions. Start here when attaching to the desktop."
    )]
    async fn list_projects(&self) -> CallToolResult {
        self.dispatch(Request::ListProjects).await
    }
    #[tool(
        description = "Create an empty editable 2D animation project. Frames are zero-based; duration is a frame count."
    )]
    async fn create_project(&self, Parameters(a): Parameters<CreateArgs>) -> CallToolResult {
        self.dispatch(Request::CreateProject {
            name: a.name,
            width: a.width,
            height: a.height,
            fps: a.fps,
            duration: a.duration,
        })
        .await
    }
    #[tool(description = "Open a .astral project at a relative workspace path.")]
    async fn open_project(&self, Parameters(a): Parameters<OpenArgs>) -> CallToolResult {
        self.dispatch(Request::OpenProject { path: a.path }).await
    }
    #[tool(
        description = "Inspect editable document, stable entity IDs, named rig joints, capabilities and optionally evaluated scene at a frame."
    )]
    async fn inspect_project(&self, Parameters(a): Parameters<InspectArgs>) -> CallToolResult {
        self.dispatch(Request::InspectProject {
            project: a.project,
            frame: a.frame,
        })
        .await
    }
    #[tool(
        description = "Validate a complete typed command batch against the expected project revision without mutating the document or history. Returns proposed counts; apply_commands still requires the current revision. Preview-generated IDs are not commit IDs."
    )]
    async fn preview_commands(&self, Parameters(batch): Parameters<Batch>) -> CallToolResult {
        self.dispatch(Request::PreviewCommands { batch }).await
    }
    #[tool(
        description = "Atomically apply typed animation commands as one undo step. Requires current expected_revision. Read inspect_project and list_presets before editing; failures commit nothing."
    )]
    async fn apply_commands(&self, Parameters(batch): Parameters<Batch>) -> CallToolResult {
        self.dispatch(Request::ApplyCommands { batch }).await
    }
    #[tool(
        description = "Save a project atomically to a relative .astral workspace path. Includes all editable actions, effects and audio references."
    )]
    async fn save_project(&self, Parameters(a): Parameters<SaveArgs>) -> CallToolResult {
        self.dispatch(Request::SaveProject {
            project: a.project,
            path: a.path,
        })
        .await
    }
    #[tool(
        description = "Render an image of a frame using the same display list as the desktop. Returns an image for visual review; maximum 32 megapixels. If a viewer locks the requested PNG, writes a fresh version and returns its actual path plus a warning; always use the returned path."
    )]
    async fn render_preview(&self, Parameters(a): Parameters<PreviewArgs>) -> CallToolResult {
        self.dispatch(Request::RenderPreview {
            project: a.project,
            frame: a.frame,
            width: a.width,
            height: a.height,
            path: a.path,
        })
        .await
    }
    #[tool(
        description = "List supported reusable action presets, effect presets, and explicit implementation limitations."
    )]
    async fn list_presets(&self) -> CallToolResult {
        self.dispatch(Request::ListPresets).await
    }
    #[tool(description = "Undo or redo one transaction, checking the current project revision.")]
    async fn history(&self, Parameters(a): Parameters<HistoryArgs>) -> CallToolResult {
        self.dispatch(if a.redo {
            Request::Redo {
                project: a.project,
                expected_revision: a.expected_revision,
            }
        } else {
            Request::Undo {
                project: a.project,
                expected_revision: a.expected_revision,
            }
        })
        .await
    }
    #[tool(
        description = "Register a workspace audio/image asset. Supported audio: WAV/AIFF/MP3/M4A. Registration alone does not place media on stage; use artwork_tools import for visible image layers or audio for full-length normalized clips, or add_audio for registered sounds."
    )]
    async fn import_asset(&self, Parameters(a): Parameters<AssetArgs>) -> CallToolResult {
        self.dispatch(Request::ImportAsset {
            project: a.project,
            expected_revision: a.expected_revision,
            path: a.path,
            tags: a.tags,
        })
        .await
    }
    #[tool(
        description = "Import a legacy file through the established source evaluator into sampled native cels, rig controls, supported skinned shapes and image quads, camera keys and embedded audio. Preserve the complete original and canonical metadata. Native source-solvers and filter/text/tiled-drawing semantics are not lossless equivalents; inspect returned conversion warnings and previews. Python/Node are transitional import dependencies."
    )]
    async fn import_legacy(&self, Parameters(a): Parameters<OpenArgs>) -> CallToolResult {
        self.dispatch(Request::ImportLegacy { path: a.path }).await
    }
    #[tool(
        description = "Start asynchronous media export with social presets. Supports native AVI/MJPEG+PCM, PNG, JPEG, PNG-sequence ZIP and WAV without FFmpeg; MP4/WebM/GIF use FFmpeg. Poll job_status."
    )]
    async fn export_video(&self, Parameters(a): Parameters<ExportArgs>) -> CallToolResult {
        self.dispatch(Request::ExportVideo {
            project: a.project,
            spec: a.spec,
        })
        .await
    }
    #[tool(
        description = "Assemble 2–32 existing local video shots into a finished MP4 with hard cuts, frame-accurate source trims in seconds, normalized sizing/FPS and synchronized stereo sound. Set clip.audio=false for silent inputs. Runs as a cancellable job; preserves the previous destination on failure. Files stay in the project workspace; does not alter the editable project."
    )]
    async fn assemble_video(&self, Parameters(a): Parameters<AssembleArgs>) -> CallToolResult {
        self.dispatch(Request::AssembleVideo {
            project: a.project,
            spec: a.spec,
        })
        .await
    }
    #[tool(
        description = "List social-media export preset IDs, formats, framing and quality choices."
    )]
    async fn list_export_presets(&self) -> CallToolResult {
        self.dispatch(Request::ListExportPresets).await
    }
    #[tool(
        description = "Export to Instagram, YouTube, TikTok or original framing. Set spec.path extension for format, preset for destination, framing fit/fill, optional width/height/fps, quality and inclusive start/end source frames. Returns a cancellable job; no automatic trimming. Native AVI is bounded to 2 GB and uses black matte for transparency."
    )]
    async fn export_media(&self, Parameters(a): Parameters<ExportArgs>) -> CallToolResult {
        self.dispatch(Request::ExportMedia {
            project: a.project,
            spec: a.spec,
        })
        .await
    }
    #[tool(
        description = "Inspect render/export job progress, warnings, errors and performance metrics."
    )]
    async fn job_status(&self, Parameters(a): Parameters<JobArgs>) -> CallToolResult {
        self.dispatch(Request::JobStatus { job: a.job }).await
    }
    #[tool(description = "Cancel an export, preserving any previous complete output file.")]
    async fn cancel_job(&self, Parameters(a): Parameters<JobArgs>) -> CallToolResult {
        self.dispatch(Request::CancelJob { job: a.job }).await
    }
    #[tool(
        description = "Recover an autosaved project by ID. Will not replace an already-open project."
    )]
    async fn recover_project(&self, Parameters(a): Parameters<ProjectArgs>) -> CallToolResult {
        self.dispatch(Request::RecoverProject { project: a.project })
            .await
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for AstralMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}
pub async fn run(service: Service, live: bool, profile: Profile) -> anyhow::Result<()> {
    let server = AstralMcp::with_profile(service, live, profile)
        .serve(rmcp::transport::stdio())
        .await?;
    server.waiting().await?;
    Ok(())
}
