use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Роль детали в комплексе памятника.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PartRole {
    Stele,
    Pedestal,
    Fence,
    FlowerBed,
    Table,
    Bench,
    Cross,
    GraveSlab,
    Other,
}

impl PartRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stele => "Стела",
            Self::Pedestal => "Тумба",
            Self::Fence => "Ограда",
            Self::FlowerBed => "Цветник",
            Self::Table => "Стол",
            Self::Bench => "Лавка",
            Self::Cross => "Крест",
            Self::GraveSlab => "Надгробие",
            Self::Other => "Другое",
        }
    }

    pub fn all() -> &'static [PartRole] {
        &[
            Self::Stele,
            Self::Pedestal,
            Self::Fence,
            Self::FlowerBed,
            Self::Table,
            Self::Bench,
            Self::Cross,
            Self::GraveSlab,
            Self::Other,
        ]
    }

    pub fn default_size_cm(self) -> SizeCm {
        match self {
            Self::Stele => SizeCm::new(60.0, 100.0, 8.0),
            Self::Pedestal => SizeCm::new(70.0, 20.0, 40.0),
            Self::Fence => SizeCm::new(200.0, 60.0, 200.0),
            Self::FlowerBed => SizeCm::new(100.0, 15.0, 50.0),
            Self::Table => SizeCm::new(50.0, 50.0, 40.0),
            Self::Bench => SizeCm::new(120.0, 45.0, 40.0),
            Self::Cross => SizeCm::new(40.0, 90.0, 8.0),
            Self::GraveSlab => SizeCm::new(80.0, 8.0, 180.0),
            Self::Other => SizeCm::new(50.0, 50.0, 50.0),
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Stele => [0.42, 0.43, 0.46],
            Self::Pedestal => [0.35, 0.36, 0.39],
            Self::Fence => [0.55, 0.52, 0.48],
            Self::FlowerBed => [0.28, 0.42, 0.30],
            Self::Table => [0.50, 0.48, 0.45],
            Self::Bench => [0.48, 0.46, 0.42],
            Self::Cross => [0.30, 0.30, 0.32],
            Self::GraveSlab => [0.40, 0.40, 0.42],
            Self::Other => [0.55, 0.50, 0.45],
        }
    }
}

/// Габариты в сантиметрах (как в заказе на памятник).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct SizeCm {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

impl SizeCm {
    pub fn new(width: f32, height: f32, depth: f32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    /// Сантиметры → метры для сцены.
    pub fn to_meters(self) -> glam::Vec3 {
        glam::Vec3::new(
            (self.width / 100.0).max(0.01),
            (self.height / 100.0).max(0.01),
            (self.depth / 100.0).max(0.01),
        )
    }
}

/// Модель в библиотеке (импорт из Blender, хранится в папке library/).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub role: PartRole,
    pub file_path: String,
    #[serde(default)]
    pub preview_path: Option<String>,
}

/// Выравнивание строк эскиза.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl TextAlign {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Слева",
            Self::Center => "По центру",
            Self::Right => "Справа",
        }
    }

    pub fn all() -> &'static [TextAlign] {
        &[Self::Left, Self::Center, Self::Right]
    }
}

/// Оформление текста гравировки.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextStyle {
    /// ID шрифта из библиотеки.
    #[serde(default)]
    pub font_id: Option<String>,
    /// Прямой путь к .ttf/.otf (если font_id не задан).
    #[serde(default)]
    pub font_path: Option<String>,
    /// Высота заглавной буквы, см.
    #[serde(default = "default_letter_cm")]
    pub letter_cm: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub align: TextAlign,
    /// Цвет дна/стенок гравировки.
    #[serde(default = "default_engrave_color")]
    pub color: [f32; 3],
    /// Глубина гравировки, мм (обычно 3–4).
    #[serde(default = "default_engrave_depth_mm")]
    pub depth_mm: f32,
    /// Межстрочный интервал (× высоты буквы).
    #[serde(default = "default_line_gap")]
    pub line_gap: f32,
}

fn default_letter_cm() -> f32 {
    4.0
}
fn default_engrave_color() -> [f32; 3] {
    [0.10, 0.10, 0.11]
}
fn default_engrave_depth_mm() -> f32 {
    3.5
}
fn default_line_gap() -> f32 {
    1.3
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_id: None,
            font_path: None,
            letter_cm: default_letter_cm(),
            bold: false,
            italic: false,
            align: TextAlign::Center,
            color: default_engrave_color(),
            depth_mm: default_engrave_depth_mm(),
            line_gap: default_line_gap(),
        }
    }
}

/// Шрифт в библиотеке.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogFont {
    pub id: String,
    pub name: String,
    pub file_path: String,
}

/// Сторона стелы, на которой расположен эскиз.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum FaceSide {
    #[default]
    Front,
    Back,
}

impl FaceSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "Лицевая",
            Self::Back => "Обратная",
        }
    }

    pub fn all() -> &'static [FaceSide] {
        &[Self::Front, Self::Back]
    }
}

/// Материал фотокерамического медальона.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum MedallionMaterial {
    /// Керамогранит
    #[default]
    CeramicGranite,
    /// Фарфор
    Porcelain,
    /// Металлокерамика
    MetalCeramic,
}

impl MedallionMaterial {
    pub fn label(self) -> &'static str {
        match self {
            Self::CeramicGranite => "Керамогранит",
            Self::Porcelain => "Фарфор",
            Self::MetalCeramic => "Металлокерамика",
        }
    }

    pub fn all() -> &'static [MedallionMaterial] {
        &[Self::CeramicGranite, Self::Porcelain, Self::MetalCeramic]
    }

    /// Толщина пластины, мм.
    pub fn thickness_mm(self) -> f32 {
        match self {
            Self::CeramicGranite => 10.0,
            Self::Porcelain => 6.0,
            Self::MetalCeramic => 1.5,
        }
    }

    /// Цвет торца / обратной стороны.
    pub fn edge_color(self) -> [f32; 3] {
        match self {
            Self::CeramicGranite => [0.82, 0.80, 0.76],
            Self::Porcelain => [0.95, 0.95, 0.93],
            Self::MetalCeramic => [0.60, 0.62, 0.64],
        }
    }
}

/// Форма медальона.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum MedallionShape {
    #[default]
    Oval,
    Rect,
    /// Бочка — прямоугольник с выпуклыми сторонами
    Barrel,
    /// Арка — прямоугольник с полукруглым верхом
    Arch,
}

impl MedallionShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Oval => "Овал",
            Self::Rect => "Прямоугольник",
            Self::Barrel => "Бочка",
            Self::Arch => "Арка",
        }
    }

    pub fn all() -> &'static [MedallionShape] {
        &[Self::Oval, Self::Rect, Self::Barrel, Self::Arch]
    }

    /// Доступна ли форма для материала.
    pub fn available_for(self, m: MedallionMaterial) -> bool {
        match self {
            Self::Oval | Self::Rect => true,
            Self::Arch => m == MedallionMaterial::CeramicGranite,
            Self::Barrel => m == MedallionMaterial::MetalCeramic,
        }
    }

    /// Стандартные размеры (ширина × высота, см) для материала.
    pub fn sizes_cm(self, m: MedallionMaterial) -> &'static [(f32, f32)] {
        use MedallionMaterial::*;
        match (self, m) {
            (Self::Oval | Self::Rect, CeramicGranite) => {
                &[(13.0, 18.0), (18.0, 24.0), (20.0, 30.0), (30.0, 40.0)]
            }
            (Self::Oval | Self::Rect, Porcelain) => &[(9.0, 12.0), (13.0, 18.0), (18.0, 24.0)],
            (Self::Oval | Self::Rect, MetalCeramic) => {
                &[(13.0, 18.0), (18.0, 24.0), (17.0, 22.0)]
            }
            (Self::Arch, CeramicGranite) => &[(13.0, 18.0), (18.0, 24.0)],
            (Self::Barrel, MetalCeramic) => &[(13.0, 18.0), (17.0, 22.0)],
            _ => &[],
        }
    }
}

/// Способ крепления медальона.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum MountType {
    /// В подготовленную нишу на клей
    #[default]
    Niche,
    /// На поверхность стелы на клей
    SurfaceGlue,
    /// На саморезы с герметизацией (металлокерамика)
    Screws,
}

impl MountType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Niche => "В нишу, на клей",
            Self::SurfaceGlue => "На поверхность, на клей",
            Self::Screws => "На саморезы",
        }
    }

    pub fn available_for(self, m: MedallionMaterial) -> bool {
        match (self, m) {
            (Self::Screws, MedallionMaterial::MetalCeramic) => true,
            (Self::Screws, _) => false,
            (_, MedallionMaterial::MetalCeramic) => false,
            _ => true,
        }
    }

    pub fn default_for(m: MedallionMaterial) -> Self {
        if m == MedallionMaterial::MetalCeramic {
            Self::Screws
        } else {
            Self::Niche
        }
    }
}

/// Подпись на медальоне (ФИО, даты).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MedallionText {
    pub content: String,
    #[serde(default)]
    pub font_id: Option<String>,
    #[serde(default)]
    pub font_path: Option<String>,
    /// Высота букв, см.
    #[serde(default = "default_medallion_letter_cm")]
    pub letter_cm: f32,
    #[serde(default = "default_medallion_text_color")]
    pub color: [f32; 3],
    #[serde(default)]
    pub align: TextAlign,
}

fn default_medallion_letter_cm() -> f32 {
    1.0
}
fn default_medallion_text_color() -> [f32; 3] {
    [0.98, 0.98, 0.98]
}

impl Default for MedallionText {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_id: None,
            font_path: None,
            letter_cm: default_medallion_letter_cm(),
            color: default_medallion_text_color(),
            align: TextAlign::Center,
        }
    }
}

/// Фотокерамический медальон.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhotoSpec {
    #[serde(default)]
    pub material: MedallionMaterial,
    #[serde(default)]
    pub shape: MedallionShape,
    #[serde(default = "default_medallion_w")]
    pub width_cm: f32,
    #[serde(default = "default_medallion_h")]
    pub height_cm: f32,
    #[serde(default)]
    pub mount: MountType,
    /// Фон из библиотеки (по форме).
    #[serde(default)]
    pub background_id: Option<String>,
    #[serde(default)]
    pub background_path: Option<String>,
    /// Вырезать человека с фото (удалить фон).
    #[serde(default = "default_true")]
    pub remove_background: bool,
    /// Допуск удаления фона 0…1.
    #[serde(default = "default_bg_tolerance")]
    pub bg_tolerance: f32,
    /// Масштаб портрета (1.0 — по высоте медальона).
    #[serde(default = "default_one")]
    pub photo_scale: f32,
    /// Смещение портрета относительно медальона (−1…1).
    #[serde(default)]
    pub photo_dx: f32,
    #[serde(default)]
    pub photo_dy: f32,
    /// Подпись (пусто — без текста).
    #[serde(default)]
    pub text: MedallionText,
}

fn default_medallion_w() -> f32 {
    13.0
}
fn default_medallion_h() -> f32 {
    18.0
}
fn default_bg_tolerance() -> f32 {
    0.28
}
fn default_one() -> f32 {
    1.0
}

impl Default for PhotoSpec {
    fn default() -> Self {
        Self {
            material: MedallionMaterial::CeramicGranite,
            shape: MedallionShape::Oval,
            width_cm: default_medallion_w(),
            height_cm: default_medallion_h(),
            mount: MountType::Niche,
            background_id: None,
            background_path: None,
            remove_background: true,
            bg_tolerance: default_bg_tolerance(),
            photo_scale: 1.0,
            photo_dx: 0.0,
            photo_dy: 0.0,
            text: MedallionText::default(),
        }
    }
}

/// Фон медальона в библиотеке (отдельная библиотека на каждую форму).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogBackground {
    pub id: String,
    pub name: String,
    pub shape: MedallionShape,
    pub file_path: String,
    #[serde(default)]
    pub preview_path: Option<String>,
}

/// Текст или фото на стеле (UV 0…1 по ширине/высоте лица).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DecalKind {
    Text {
        content: String,
        /// Устарело: относительный кегль; используется если style.letter_cm не задан явно.
        #[serde(default = "default_legacy_font_scale")]
        font_scale: f32,
        #[serde(default)]
        style: TextStyle,
    },
    Photo {
        path: String,
        #[serde(default)]
        spec: PhotoSpec,
    },
}

fn default_legacy_font_scale() -> f32 {
    0.55
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decal {
    pub id: String,
    /// Имя слоя (как в Photoshop).
    #[serde(default = "default_decal_name")]
    pub name: String,
    pub kind: DecalKind,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Сторона стелы.
    #[serde(default)]
    pub side: FaceSide,
    /// Центр на лице стелы: u = 0 слева, 1 справа; v = 0 снизу, 1 сверху.
    pub u: f32,
    pub v: f32,
    /// Ширина/высота декали относительно лица стелы (0…1).
    pub width: f32,
    pub height: f32,
}

fn default_decal_name() -> String {
    "Эскиз".into()
}

fn default_true() -> bool {
    true
}

impl Decal {
    pub fn new_text(content: impl Into<String>) -> Self {
        let content = content.into();
        let name = content
            .lines()
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.chars().count() > 24 {
                    format!("{}…", s.chars().take(24).collect::<String>())
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "Текст".into());
        Self {
            id: new_id(),
            name,
            kind: DecalKind::Text {
                content,
                font_scale: 0.55,
                style: TextStyle::default(),
            },
            visible: true,
            side: FaceSide::Front,
            u: 0.5,
            v: 0.42,
            width: 0.78,
            height: 0.32,
        }
    }

    pub fn new_photo(path: impl Into<String>) -> Self {
        let path = path.into();
        let name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Фото")
            .to_string();
        Self {
            id: new_id(),
            name,
            kind: DecalKind::Photo {
                path,
                spec: PhotoSpec::default(),
            },
            visible: true,
            side: FaceSide::Front,
            u: 0.5,
            v: 0.72,
            width: 0.42,
            height: 0.32,
        }
    }

    /// Фото-медальон: вернуть спецификацию, если это фото.
    pub fn photo_spec(&self) -> Option<&PhotoSpec> {
        match &self.kind {
            DecalKind::Photo { spec, .. } => Some(spec),
            _ => None,
        }
    }

    pub fn layer_label(&self) -> String {
        let kind = match &self.kind {
            DecalKind::Text { .. } => "Т",
            DecalKind::Photo { .. } => "Ф",
        };
        format!("[{kind}] {}", self.name)
    }
}

/// Экземпляр детали на участке.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedPart {
    pub id: String,
    pub name: String,
    pub role: PartRole,
    /// ID группы памятника (несколько памятников на одном участке).
    pub monument_id: String,
    /// Путь к .obj/.gltf/.glb. Если None — габаритная заглушка точного размера.
    pub model_path: Option<String>,
    pub size: SizeCm,
    /// Равномерный масштаб поверх размеров (1.0 = как указано в см).
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Смещение по земле в метрах (относительно центра группы).
    pub offset_x: f32,
    /// Смещение по высоте (м). Отрицательное — уходит под землю (обрезается).
    #[serde(default)]
    pub offset_y: f32,
    pub offset_z: f32,
    pub yaw_deg: f32,
    /// Если true — стела/крест ставятся на верх тумбы той же группы.
    pub auto_stack: bool,
    /// Путь к текстуре из библиотеки (albedo).
    #[serde(default)]
    pub texture_path: Option<String>,
    pub decals: Vec<Decal>,
    pub visible: bool,
}

fn default_scale() -> f32 {
    1.0
}

/// Категория текстуры (своя «библиотека» внутри каталога текстур).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TextureCategory {
    pub id: String,
    pub name: String,
}

impl TextureCategory {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
        }
    }
}

/// Текстура в библиотеке.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogTexture {
    pub id: String,
    pub name: String,
    /// ID категории (библиотеки текстур). None — без категории.
    #[serde(default)]
    pub category_id: Option<String>,
    pub file_path: String,
    #[serde(default)]
    pub preview_path: Option<String>,
}

impl PlacedPart {
    pub fn new(role: PartRole, monument_id: impl Into<String>) -> Self {
        let monument_id = monument_id.into();
        Self {
            id: new_id(),
            name: role.label().to_string(),
            role,
            monument_id,
            model_path: None,
            size: role.default_size_cm(),
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            offset_z: 0.0,
            yaw_deg: 0.0,
            auto_stack: matches!(role, PartRole::Stele | PartRole::Cross),
            texture_path: None,
            decals: vec![],
            visible: true,
        }
    }

    /// Габариты в метрах с учётом масштаба.
    pub fn effective_size_m(&self) -> glam::Vec3 {
        let s = self.scale.clamp(0.05, 10.0);
        self.size.to_meters() * s
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monument {
    pub id: String,
    pub name: String,
    /// Позиция группы на участке (метры).
    pub x: f32,
    pub z: f32,
}

impl Monument {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            x: 0.0,
            z: 0.0,
        }
    }
}

/// Размер участка в сантиметрах (ширина × длина).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlotSizeCm {
    pub width: f32,
    pub length: f32,
}

impl PlotSizeCm {
    pub fn new(width: f32, length: f32) -> Self {
        Self { width, length }
    }

    pub fn label(self) -> String {
        format!(
            "{:.0}×{:.0} см",
            self.width.round(),
            self.length.round()
        )
    }

    /// Метры: X = ширина, Z = длина.
    pub fn to_meters(self) -> (f32, f32) {
        ((self.width / 100.0).max(0.5), (self.length / 100.0).max(0.5))
    }

    pub fn presets() -> &'static [PlotSizeCm] {
        const PRESETS: &[PlotSizeCm] = &[
            PlotSizeCm { width: 120.0, length: 180.0 },
            PlotSizeCm { width: 180.0, length: 200.0 },
            PlotSizeCm { width: 240.0, length: 180.0 },
            PlotSizeCm { width: 200.0, length: 200.0 },
            PlotSizeCm { width: 210.0, length: 180.0 },
            PlotSizeCm { width: 240.0, length: 220.0 },
            PlotSizeCm { width: 300.0, length: 240.0 },
        ];
        PRESETS
    }
}

impl Default for PlotSizeCm {
    fn default() -> Self {
        Self::new(180.0, 200.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub file_path: Option<String>,
    pub monuments: Vec<Monument>,
    pub parts: Vec<PlacedPart>,
    /// Устарело: библиотека теперь глобальная в AppData / папке library/.
    #[serde(default)]
    pub catalog: Vec<CatalogModel>,
    pub active_monument_id: Option<String>,
    pub active_part_id: Option<String>,
    pub ground: bool,
    /// Размер участка (см).
    #[serde(default)]
    pub plot_size: PlotSizeCm,
    /// Текстура покрытия участка (из библиотеки текстур).
    #[serde(default)]
    pub plot_texture_path: Option<String>,
}

pub fn create_project(name: impl Into<String>) -> Project {
    let now = Utc::now();
    let monument = Monument::new("Памятник 1");
    let mid = monument.id.clone();

    let pedestal = PlacedPart::new(PartRole::Pedestal, &mid);
    let mut stele = PlacedPart::new(PartRole::Stele, &mid);
    stele.decals.push(Decal::new_text("Иванов\nИван Иванович\n1950 — 2020"));
    let stele_id = stele.id.clone();

    Project {
        id: new_id(),
        name: name.into(),
        description: String::new(),
        created_at: now,
        updated_at: now,
        file_path: None,
        monuments: vec![monument],
        parts: vec![pedestal, stele],
        catalog: vec![],
        active_monument_id: Some(mid),
        active_part_id: Some(stele_id),
        ground: true,
        plot_size: PlotSizeCm::default(),
        plot_texture_path: None,
    }
}
