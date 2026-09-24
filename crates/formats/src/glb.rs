//! Lectura de `.glb` (glTF 2.0 binario) con el crate `gltf`. glTF ya usa el espacio de
//! Revvy: Y arriba, +Z adelante, metros. Acá no hay traducción, solo lectura.

use std::path::Path;

use glam::{Mat3, Mat4, Vec3};

use crate::layout::SurfaceType;
use crate::mesh::VisualMesh;
use crate::ncp::CollisionTri;
use crate::FormatError;

/// Luz fija con la que se hornea el color de las mallas `.glb`. El render dibuja
/// `textura × color de vértice`, como el gouraud de las pistas de Re-Volt.
const SUN: Vec3 = Vec3::new(0.25, 0.92, 0.2);
const AMBIENT: f32 = 0.55;

pub struct GlbFile {
    pub nodes: Vec<GlbNode>,
    pub images: Vec<image::RgbaImage>,
}

pub struct GlbNode {
    pub name: String,
    /// Nombres de los nodos padre, de la raíz hacia abajo.
    pub ancestors: Vec<String>,
    /// Del nodo a la escena.
    pub world: Mat4,
    pub primitives: Vec<GlbPrimitive>,
}

/// Una primitiva de triángulos en el espacio del nodo.
pub struct GlbPrimitive {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub material: Option<String>,
    pub base_color: [f32; 4],
    /// Índice en `GlbFile::images` de la textura base.
    pub image: Option<usize>,
}

impl GlbNode {
    /// El nodo o alguno de sus padres se llama `name`.
    pub fn is_under(&self, name: &str) -> bool {
        self.name == name || self.ancestors.iter().any(|ancestor| ancestor == name)
    }
}

pub fn read(path: &Path) -> Result<GlbFile, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(|err| FormatError::parse(path, err.to_string()))?;
    let blob = gltf.blob.as_deref();

    let mut images = Vec::new();
    for image in gltf.images() {
        let decoded = match image.source() {
            gltf::image::Source::View { view, .. } => blob
                .and_then(|blob| blob.get(view.offset()..view.offset() + view.length()))
                .and_then(|data| image::load_from_memory(data).ok())
                .map(|img| img.to_rgba8()),
            gltf::image::Source::Uri { .. } => None,
        };
        images.push(decoded.unwrap_or_else(|| {
            tracing::warn!(path = %path.display(), "textura de .glb ilegible o externa");
            image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]))
        }));
    }

    let mut nodes = Vec::new();
    let scene = gltf.default_scene().or_else(|| gltf.scenes().next());
    if let Some(scene) = scene {
        for node in scene.nodes() {
            visit(&node, Mat4::IDENTITY, &mut Vec::new(), blob, &mut nodes);
        }
    }
    Ok(GlbFile { nodes, images })
}

fn visit(node: &gltf::Node, parent: Mat4, ancestors: &mut Vec<String>, blob: Option<&[u8]>, out: &mut Vec<GlbNode>) {
    let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    let name = node.name().unwrap_or_default().to_string();
    let mut primitives = Vec::new();
    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buffer| match buffer.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let positions: Vec<Vec3> = positions.map(Vec3::from).collect();
            let normals = reader
                .read_normals()
                .map(|normals| normals.map(Vec3::from).collect())
                .unwrap_or_default();
            let uvs = reader
                .read_tex_coords(0)
                .map(|uvs| uvs.into_f32().collect())
                .unwrap_or_default();
            let colors = reader
                .read_colors(0)
                .map(|colors| colors.into_rgba_f32().collect())
                .unwrap_or_default();
            let indices = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            primitives.push(GlbPrimitive {
                positions,
                normals,
                uvs,
                colors,
                indices,
                material: material.name().map(str::to_string),
                base_color: pbr.base_color_factor(),
                image: pbr.base_color_texture().map(|info| info.texture().source().index()),
            });
        }
    }
    out.push(GlbNode {
        name: name.clone(),
        ancestors: ancestors.clone(),
        world,
        primitives,
    });
    ancestors.push(name);
    for child in node.children() {
        visit(&child, world, ancestors, blob, out);
    }
    ancestors.pop();
}

/// Mallas visibles: cada primitiva con la matriz `to_space`, el color base por vértice y
/// una luz fija horneada. La página de textura es el índice de la imagen, o -1.
pub fn visual_meshes(nodes: &[&GlbNode], to_space: impl Fn(&GlbNode) -> Mat4) -> Vec<VisualMesh> {
    let mut meshes = Vec::new();
    let sun = SUN.normalize();
    for node in nodes {
        let matrix = to_space(node);
        let normal_matrix = Mat3::from_mat4(matrix).inverse().transpose();
        for primitive in &node.primitives {
            let positions: Vec<Vec3> = primitive.positions.iter().map(|&p| matrix.transform_point3(p)).collect();
            let normals: Vec<Vec3> = if primitive.normals.len() == positions.len() {
                primitive
                    .normals
                    .iter()
                    .map(|&n| (normal_matrix * n).normalize_or_zero())
                    .collect()
            } else {
                flat_normals(&positions, &primitive.indices)
            };
            let colors = normals
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let vertex = primitive.colors.get(i).copied().unwrap_or([1.0; 4]);
                    let light = AMBIENT + (1.0 - AMBIENT) * n.dot(sun).max(0.0);
                    let channel = |c: usize| (primitive.base_color[c] * vertex[c] * light * 255.0).clamp(0.0, 255.0) as u8;
                    [channel(0), channel(1), channel(2), 255]
                })
                .collect();
            let uvs = if primitive.uvs.len() == positions.len() {
                primitive.uvs.clone()
            } else {
                vec![[0.0, 0.0]; positions.len()]
            };
            meshes.push(VisualMesh {
                name: node.name.clone(),
                texture_page: primitive.image.map_or(-1, |image| image as i16),
                positions,
                normals,
                uvs,
                colors,
                indices: primitive.indices.clone(),
            });
        }
    }
    meshes
}

/// Triángulos de colisión en el espacio de la escena. El nombre del material es la
/// superficie (`Road`, `Ice`, …); uno desconocido cae en `Road`.
pub fn collision_triangles(nodes: &[&GlbNode]) -> Vec<CollisionTri> {
    let mut triangles = Vec::new();
    let mut unknown = std::collections::BTreeSet::new();
    for node in nodes {
        for primitive in &node.primitives {
            let surface = match primitive.material.as_deref() {
                Some(name) => SurfaceType::from_name(name).unwrap_or_else(|| {
                    unknown.insert(name.to_string());
                    SurfaceType::Road
                }),
                None => SurfaceType::Road,
            };
            for tri in primitive.indices.chunks_exact(3) {
                let corner = |i: u32| node.world.transform_point3(primitive.positions[i as usize]);
                triangles.push(CollisionTri {
                    positions: [corner(tri[0]), corner(tri[1]), corner(tri[2])],
                    surface,
                    camera_only: false,
                    object_only: false,
                });
            }
        }
    }
    if !unknown.is_empty() {
        tracing::warn!(materiales = ?unknown, "material de Collision sin superficie conocida: se usa Road");
    }
    triangles
}

fn flat_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| positions[i as usize]);
        let n = (b - a).cross(c - a);
        for &i in tri {
            normals[i as usize] += n;
        }
    }
    normals.iter().map(|n| n.normalize_or_zero()).collect()
}
