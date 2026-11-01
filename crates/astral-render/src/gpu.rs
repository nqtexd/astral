//! Offscreen wgpu renderer for the same vector/textured display list as the viewport.
use crate::*;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;
struct CachedPath {
    contours: Vec<Vec<Point>>,
    points: Vec<Point>,
    indices: Vec<u32>,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}
struct Texture {
    group: wgpu::BindGroup,
    _texture: wgpu::Texture,
    _image: Arc<DecodedBitmap>,
}
struct Draw {
    start: u32,
    end: u32,
    texture: usize,
    overwrite: bool,
}
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    overwrite: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    textures: HashMap<usize, Texture>,
    paths: HashMap<u64, CachedPath>,
    path_bytes: usize,
    white: Arc<DecodedBitmap>,
    output: wgpu::Texture,
    msaa: wgpu::Texture,
    readback: wgpu::Buffer,
    width: u32,
    height: u32,
    padded: u32,
    pub adapter: String,
    pub readback_seconds: f64,
}
const SHADER: &str = r#"
struct Scene { size:vec2<f32>, clip_min:vec2<f32>, clip_max:vec2<f32>, padding:vec2<f32> };
@group(0) @binding(0) var<uniform> scene:Scene;
@group(0) @binding(1) var image:texture_2d<f32>;
@group(0) @binding(2) var image_sampler:sampler;
struct Out {@builtin(position) position:vec4<f32>,@location(0) uv:vec2<f32>,@location(1) color:vec4<f32>};
@vertex fn vs(@location(0) position:vec2<f32>,@location(1) uv:vec2<f32>,@location(2) color:vec4<f32>)->Out {
 var out:Out;out.position=vec4<f32>(position.x/scene.size.x*2.0-1.0,1.0-position.y/scene.size.y*2.0,0.0,1.0);out.uv=uv;out.color=color;return out;
}
@fragment fn fs(in:Out)->@location(0) vec4<f32>{
 if(any(in.position.xy<scene.clip_min)||any(in.position.xy>=scene.clip_max)){discard;}
 return textureSample(image,image_sampler,in.uv)*in.color;
}
"#;
impl GpuRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        pollster::block_on(Self::create(width, height))
    }
    async fn create(width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0
                && height > 0
                && width <= 8192
                && height <= 8192
                && width as u64 * height as u64 <= 33_554_432,
            "GPU output exceeds pixel budget"
        );
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await?;
        ensure!(
            adapter.get_info().device_type != wgpu::DeviceType::Cpu,
            "Only a software adapter is available"
        );
        ensure!(
            adapter
                .get_texture_format_features(wgpu::TextureFormat::Rgba8Unorm)
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4),
            "Adapter lacks 4x multisampling"
        );
        let name = format!(
            "{} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Astral export GPU"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await?;
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Astral export scene"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Astral shared artwork shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make_pipeline = |blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("Astral export rasterization"),layout:Some(&pipeline_layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Vertex>()as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4]})]},primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:None,multisample:wgpu::MultisampleState{count:4,..Default::default()},fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format:wgpu::TextureFormat::Rgba8Unorm,blend,write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None})
        };
        let pipeline = make_pipeline(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let overwrite = make_pipeline(None);
        let texture = |samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Astral export surface"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage,
                view_formats: &[],
            })
        };
        let output = texture(
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa = texture(4, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let padded = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Astral export readback"),
            size: padded as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            overwrite,
            layout,
            uniform,
            sampler,
            textures: HashMap::new(),
            paths: HashMap::new(),
            path_bytes: 0,
            white: Arc::new(DecodedBitmap {
                id: Id::nil(),
                width: 1,
                height: 1,
                pixels: vec![255; 4],
            }),
            output,
            msaa,
            readback,
            width,
            height,
            padded,
            adapter: name,
            readback_seconds: 0.,
        })
    }
    fn texture(&mut self, image: &Arc<DecodedBitmap>) -> usize {
        let key = Arc::as_ptr(image) as usize;
        if self.textures.contains_key(&key) {
            return key;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Astral artwork texture"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.textures.insert(
            key,
            Texture {
                group,
                _texture: texture,
                _image: image.clone(),
            },
        );
        key
    }
    pub fn render(&mut self, list: &DisplayList, fill: bool) -> Result<Vec<u8>> {
        if self
            .textures
            .values()
            .map(|t| t._image.pixels.len())
            .sum::<usize>()
            > 128 * 1024 * 1024
        {
            self.textures.clear();
        }
        let sx = self.width as f32 / list.width as f32;
        let sy = self.height as f32 / list.height as f32;
        let scale = if fill { sx.max(sy) } else { sx.min(sy) };
        let x = (self.width as f32 - list.width as f32 * scale) / 2.;
        let y = (self.height as f32 - list.height as f32 * scale) / 2.;
        let clip = [
            x.max(0.),
            y.max(0.),
            (x + list.width as f32 * scale).min(self.width as f32),
            (y + list.height as f32 * scale).min(self.height as f32),
        ];
        let uniform = [
            self.width as f32,
            self.height as f32,
            clip[0],
            clip[1],
            clip[2],
            clip[3],
            0.,
            0.,
        ];
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        let white = self.white.clone();
        let white_id = self.texture(&white);
        let mut vertices = vec![];
        let mut indices = vec![];
        let mut draws: Vec<Draw> = vec![];
        let mut append = |points: &[Point],
                          uvs: Option<&[Point]>,
                          triangles: &[u32],
                          color: [f32; 4],
                          texture: usize,
                          overwrite: bool| {
            let offset = vertices.len() as u32;
            let start = indices.len() as u32;
            vertices.extend(points.iter().enumerate().map(|(i, p)| Vertex {
                position: [p.x * scale + x, p.y * scale + y],
                uv: uvs.map_or([0.5, 0.5], |u| [u[i].x, u[i].y]),
                color,
            }));
            indices.extend(triangles.iter().map(|i| *i + offset));
            let end = indices.len() as u32;
            if let Some(last) = draws.last_mut()
                && !overwrite
                && !last.overwrite
                && last.texture == texture
            {
                last.end = end;
                return;
            }
            draws.push(Draw {
                start,
                end,
                texture,
                overwrite,
            });
        };
        append(
            &[
                Point::new(0., 0.),
                Point::new(list.width as f32, 0.),
                Point::new(list.width as f32, list.height as f32),
                Point::new(0., list.height as f32),
            ],
            None,
            &[0, 1, 2, 0, 2, 3],
            premult(list.background),
            white_id,
            true,
        );
        for primitive in &list.primitives {
            match primitive {
                Primitive::Path { contours, color } => {
                    use std::hash::{Hash, Hasher};
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    for contour in contours {
                        contour.len().hash(&mut hash);
                        for point in contour {
                            point.x.to_bits().hash(&mut hash);
                            point.y.to_bits().hash(&mut hash);
                        }
                    }
                    let hash = hash.finish();
                    if !self
                        .paths
                        .get(&hash)
                        .is_some_and(|entry| entry.contours == *contours)
                    {
                        let (points, indices) = tessellate(contours)?;
                        let bytes = contours
                            .iter()
                            .map(|c| c.len() * std::mem::size_of::<Point>())
                            .sum::<usize>()
                            + points.len() * std::mem::size_of::<Point>()
                            + indices.len() * 4;
                        if self.paths.len() >= 512 || self.path_bytes + bytes > 32 * 1024 * 1024 {
                            self.paths.clear();
                            self.path_bytes = 0;
                        }
                        self.path_bytes += bytes;
                        self.paths.insert(
                            hash,
                            CachedPath {
                                contours: contours.clone(),
                                points,
                                indices,
                            },
                        );
                    }
                    let entry = &self.paths[&hash];
                    append(
                        &entry.points,
                        None,
                        &entry.indices,
                        premult(*color),
                        white_id,
                        false,
                    );
                }
                Primitive::Image {
                    bitmap,
                    vertices: v,
                    indices: i,
                    opacity,
                } => {
                    let texture = self.texture(bitmap);
                    let points: Vec<_> = v.iter().map(|v| v.position).collect();
                    let uv: Vec<_> = v.iter().map(|v| v.uv).collect();
                    append(&points, Some(&uv), i, [*opacity; 4], texture, false);
                }
                _ => {
                    let (points, triangles, color) = vector_mesh(primitive)?;
                    append(&points, None, &triangles, premult(color), white_id, false);
                }
            }
        }
        let vb = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let ib = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let output = self.output.create_view(&Default::default());
        let msaa = self.msaa.create_view(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Astral export"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa,
                    depth_slice: None,
                    resolve_target: Some(&output),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                ..Default::default()
            });
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            for draw in &draws {
                pass.set_pipeline(if draw.overwrite {
                    &self.overwrite
                } else {
                    &self.pipeline
                });
                pass.set_bind_group(0, &self.textures[&draw.texture].group, &[]);
                pass.draw_indexed(draw.start..draw.end, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        let tick = Instant::now();
        let submission = self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })?;
        rx.recv_timeout(Duration::from_secs(30))??;
        let view = self.readback.slice(..).get_mapped_range()?;
        let mut rgba = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for row in view.chunks(self.padded as usize).take(self.height as usize) {
            rgba.extend_from_slice(&row[..self.width as usize * 4]);
        }
        drop(view);
        self.readback.unmap();
        self.readback_seconds += tick.elapsed().as_secs_f64();
        for pixel in rgba.as_chunks_mut::<4>().0 {
            if pixel[3] == 0 {
                pixel[..3].fill(0);
            } else if pixel[3] < 255 {
                for c in 0..3 {
                    pixel[c] = ((pixel[c] as u32 * 255 + pixel[3] as u32 / 2) / pixel[3] as u32)
                        .min(255) as u8;
                }
            }
        }
        Ok(rgba)
    }
}
fn premult(color: Color) -> [f32; 4] {
    let alpha = color[3] as f32 / 255.;
    [
        color[0] as f32 / 255. * alpha,
        color[1] as f32 / 255. * alpha,
        color[2] as f32 / 255. * alpha,
        alpha,
    ]
}
fn vector_mesh(primitive: &Primitive) -> Result<(Vec<Point>, Vec<u32>, Color)> {
    use lyon_tessellation::{
        BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
        math::point,
        path::{LineCap, LineJoin, Path},
    };
    return match primitive {
        Primitive::Path { contours, color } => {
            let (v, i) = tessellate(contours)?;
            Ok((v, i, *color))
        }
        Primitive::Circle {
            center,
            radius,
            color,
        } => {
            let contour = (0..64)
                .map(|i| {
                    let a = i as f32 / 64. * std::f32::consts::TAU;
                    *center + Point::new(a.cos() * radius, a.sin() * radius)
                })
                .collect();
            let (v, i) = tessellate(&[contour])?;
            Ok((v, i, *color))
        }
        Primitive::Line { a, b, width, color } => stroke_mesh(&[*a, *b], *width, *color),
        Primitive::Polyline {
            points,
            width,
            color,
        } => stroke_mesh(points, *width, *color),
        Primitive::Image { .. } => unreachable!(),
    };
    fn stroke_mesh(
        points: &[Point],
        width: f32,
        color: Color,
    ) -> Result<(Vec<Point>, Vec<u32>, Color)> {
        if points.len() < 2 {
            return Ok((vec![], vec![], color));
        }
        let mut builder = Path::builder();
        builder.begin(point(points[0].x, points[0].y));
        for p in &points[1..] {
            builder.line_to(point(p.x, p.y));
        }
        builder.end(false);
        let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
        StrokeTessellator::new()
            .tessellate_path(
                &builder.build(),
                &StrokeOptions::default()
                    .with_line_width(width)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round),
                &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex| {
                    let q = v.position();
                    Point::new(q.x, q.y)
                }),
            )
            .map_err(|e| anyhow::anyhow!("Stroke tessellation: {e:?}"))?;
        Ok((buffers.vertices, buffers.indices, color))
    }
}
