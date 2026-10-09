use crate::library::{self, LibraryStore, PreviewCache};
use crate::model::*;
use crate::persist::{self, AppData};
use crate::scene::{assemble_scene, OrbitCamera, SteleFace, TextureCache, TriangleMesh};
use crate::ui::{self, PhotoEditorState, Screen, TextEditorState};
use eframe::egui;

#[derive(Clone, Debug)]
pub enum ConfirmAction {
    DeletePart { id: String, name: String },
    DeleteMonument { id: String, name: String },
    DeleteLibraryItem { id: String, name: String },
    DeleteTexture { id: String, name: String },
    DeleteTextureCategory { id: String, name: String },
    DeleteFont { id: String, name: String },
    DeleteBackground { id: String, name: String },
    DeleteProject { id: String, name: String },
}

/// Куда назначить выбранную текстуру.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureTarget {
    ActivePart,
    Plot,
}

/// Модальное окно выбора из библиотеки.
#[derive(Clone, Debug)]
pub enum LibraryPicker {
    /// Каталог моделей, отфильтрованный по роли активной детали.
    Models { role: PartRole },
    /// Каталог текстур (опционально по категории).
    Textures {
        category_id: Option<String>,
        target: TextureTarget,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Models,
    Textures,
    Fonts,
    Backgrounds,
}

pub struct StelaApp {
    pub screen: Screen,
    pub data: AppData,
    pub library: LibraryStore,
    pub preview_cache: PreviewCache,
    pub camera: OrbitCamera,
    pub meshes: Vec<TriangleMesh>,
    pub stele_faces: Vec<SteleFace>,
    pub build_warnings: Vec<String>,
    pub rebuild_needed: bool,
    pub frame_camera: bool,
    pub textures: TextureCache,
    pub new_project_name: String,
    pub status: String,
    pub draft_text: String,
    pub catalog_role: PartRole,
    pub confirm: Option<ConfirmAction>,
    pub new_model_name: String,
    pub new_texture_name: String,
    pub new_texture_category_name: String,
    pub new_font_name: String,
    pub new_background_name: String,
    pub background_shape: MedallionShape,
    pub library_tab: LibraryTab,
    pub texture_category_filter: Option<String>,
    pub library_picker: Option<LibraryPicker>,
    /// Выбранный слой эскиза на стеле (как в Photoshop).
    pub active_decal_id: Option<String>,
    /// Окно редактора текста / гравировки.
    pub text_editor: Option<TextEditorState>,
    pub photo_editor: Option<PhotoEditorState>,
}

impl Default for StelaApp {
    fn default() -> Self {
        Self {
            screen: Screen::Home,
            data: persist::load(),
            library: library::load_library(),
            preview_cache: PreviewCache::default(),
            camera: OrbitCamera::default(),
            meshes: vec![],
            stele_faces: vec![],
            build_warnings: vec![],
            rebuild_needed: true,
            frame_camera: true,
            textures: TextureCache::default(),
            new_project_name: "Участок 1".into(),
            status: "Готово".into(),
            draft_text: "Фамилия Имя Отчество\n1950 — 2020".into(),
            catalog_role: PartRole::Stele,
            confirm: None,
            new_model_name: String::new(),
            new_texture_name: String::new(),
            new_texture_category_name: String::new(),
            new_font_name: String::new(),
            new_background_name: String::new(),
            background_shape: MedallionShape::Oval,
            library_tab: LibraryTab::Models,
            texture_category_filter: None,
            library_picker: None,
            active_decal_id: None,
            text_editor: None,
            photo_editor: None,
        }
    }
}

impl StelaApp {
    pub fn active_project(&self) -> Option<&Project> {
        let id = self.data.active_project_id.as_ref()?;
        self.data.projects.iter().find(|p| &p.id == id)
    }

    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        let id = self.data.active_project_id.clone()?;
        self.data.projects.iter_mut().find(|p| p.id == id)
    }

    pub fn active_part(&self) -> Option<&PlacedPart> {
        let p = self.active_project()?;
        let id = p.active_part_id.as_ref()?;
        p.parts.iter().find(|x| &x.id == id)
    }

    pub fn persist(&self) {
        persist::save(&self.data);
        library::save_library(&self.library);
    }

    pub fn touch(&mut self) {
        if let Some(p) = self.active_project_mut() {
            p.updated_at = chrono::Utc::now();
        }
        self.rebuild_needed = true;
        self.persist();
    }

    pub fn create_project_from_dialog(&mut self) {
        let name = {
            let t = self.new_project_name.trim();
            if t.is_empty() {
                "Участок".to_string()
            } else {
                t.to_string()
            }
        };
        let safe_file = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .replace(' ', "_");

        let Some(path) = rfd::FileDialog::new()
            .set_title("Создать проект Stela — куда сохранить")
            .set_file_name(format!("{safe_file}.stela.json"))
            .add_filter("Проект Stela", &["json"])
            .save_file()
        else {
            self.status = "Создание отменено".into();
            return;
        };

        let mut project = create_project(name);
        project.file_path = Some(path.to_string_lossy().into_owned());
        if let Err(e) = persist::export_project(&project, &path) {
            self.status = format!("Не удалось сохранить: {e}");
            return;
        }
        let id = project.id.clone();
        let shown = path.display().to_string();
        self.data.projects.insert(0, project);
        self.data.active_project_id = Some(id);
        self.rebuild_needed = true;
        self.frame_camera = true;
        self.camera = OrbitCamera::default();
        self.screen = Screen::Studio;
        self.persist();
        self.status = format!("Проект создан: {shown}");
    }

    pub fn open_project(&mut self, id: &str) {
        self.data.active_project_id = Some(id.to_string());
        self.rebuild_needed = true;
        self.frame_camera = true;
        self.camera = OrbitCamera::default();
        self.screen = Screen::Studio;
        self.persist();
    }

    pub fn request_delete_project(&mut self, id: &str) {
        let name = self
            .data
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "проект".into());
        self.confirm = Some(ConfirmAction::DeleteProject {
            id: id.to_string(),
            name,
        });
    }

    pub fn rebuild_scene(&mut self) {
        if let Some(p) = self.active_project().cloned() {
            let built = assemble_scene(&p);
            self.meshes = built.meshes;
            self.stele_faces = built.stele_faces;
            self.build_warnings = built.warnings;
            if self.frame_camera {
                if let Some(max_y) = self
                    .meshes
                    .iter()
                    .flat_map(|m| m.positions.iter())
                    .map(|v| v.y)
                    .reduce(f32::max)
                {
                    self.camera.frame_height(max_y);
                }
                self.frame_camera = false;
            }
        } else {
            self.meshes.clear();
            self.stele_faces.clear();
        }
        self.rebuild_needed = false;
    }

    pub fn add_monument(&mut self) {
        let Some(p) = self.active_project_mut() else {
            return;
        };
        let n = p.monuments.len() + 1;
        let mut m = Monument::new(format!("Памятник {n}"));
        m.x = (n as f32 - 1.0) * 2.5;
        let mid = m.id.clone();
        p.monuments.push(m);
        p.active_monument_id = Some(mid.clone());

        let pedestal = PlacedPart::new(PartRole::Pedestal, &mid);
        let mut stele = PlacedPart::new(PartRole::Stele, &mid);
        stele.decals.push(Decal::new_text("Фамилия Имя\n0000 — 0000"));
        let sid = stele.id.clone();
        p.parts.push(pedestal);
        p.parts.push(stele);
        p.active_part_id = Some(sid);
        self.frame_camera = true;
        self.touch();
        self.status = "Добавлен памятник".into();
    }

    pub fn request_delete_active_monument(&mut self) {
        let Some(p) = self.active_project() else {
            return;
        };
        let Some(mid) = p.active_monument_id.clone() else {
            return;
        };
        if p.monuments.len() <= 1 {
            self.status = "Нельзя удалить единственный памятник".into();
            return;
        }
        let name = p
            .monuments
            .iter()
            .find(|m| m.id == mid)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "памятник".into());
        self.confirm = Some(ConfirmAction::DeleteMonument { id: mid, name });
    }

    pub fn add_part(&mut self, role: PartRole) {
        let mid = self
            .active_project()
            .and_then(|p| p.active_monument_id.clone())
            .or_else(|| {
                self.active_project()
                    .and_then(|p| p.monuments.first().map(|m| m.id.clone()))
            });
        let Some(mid) = mid else {
            self.status = "Сначала создайте памятник".into();
            return;
        };
        let mut part = PlacedPart::new(role, mid);
        if !matches!(role, PartRole::Stele | PartRole::Pedestal | PartRole::Cross) {
            part.offset_x = match role {
                PartRole::Table => 1.2,
                PartRole::Bench => -1.2,
                PartRole::FlowerBed => 0.0,
                PartRole::Fence => 0.0,
                _ => 1.5,
            };
            part.offset_z = match role {
                PartRole::FlowerBed => 0.9,
                PartRole::Fence => 0.0,
                PartRole::Table | PartRole::Bench => 0.6,
                _ => 1.0,
            };
            part.auto_stack = false;
        }
        let id = part.id.clone();
        if let Some(p) = self.active_project_mut() {
            p.parts.push(part);
            p.active_part_id = Some(id);
        }
        self.frame_camera = true;
        self.touch();
        self.status = format!("Добавлено: {}", role.label());
    }

    pub fn add_part_from_catalog(&mut self, catalog_id: &str) {
        let item = self
            .library
            .items
            .iter()
            .find(|c| c.id == catalog_id)
            .cloned();
        let Some(item) = item else {
            return;
        };
        let mid = self
            .active_project()
            .and_then(|p| p.active_monument_id.clone())
            .or_else(|| {
                self.active_project()
                    .and_then(|p| p.monuments.first().map(|m| m.id.clone()))
            });
        let Some(mid) = mid else {
            self.status = "Откройте проект, чтобы добавить модель на участок".into();
            return;
        };
        let mut part = PlacedPart::new(item.role, mid);
        part.name = item.name.clone();
        part.model_path = Some(item.file_path);
        if !matches!(item.role, PartRole::Stele | PartRole::Pedestal | PartRole::Cross) {
            part.offset_z = 1.0;
            part.auto_stack = false;
        }
        let id = part.id.clone();
        let name = item.name;
        if let Some(p) = self.active_project_mut() {
            p.parts.push(part);
            p.active_part_id = Some(id);
        }
        self.frame_camera = true;
        self.touch();
        self.status = format!("На участок: {name}");
    }

    pub fn request_delete_active_part(&mut self) {
        if let Some(part) = self.active_part() {
            self.confirm = Some(ConfirmAction::DeletePart {
                id: part.id.clone(),
                name: part.name.clone(),
            });
        }
    }

    pub fn request_delete_on_hotkey(&mut self) {
        // Prefer deleting selected part; if none — monument
        if self.active_part().is_some() {
            self.request_delete_active_part();
        } else {
            self.request_delete_active_monument();
        }
    }

    pub fn import_model_for_active_part(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Импорт 3D-модели (Blender → OBJ / glTF / GLB)")
            .add_filter("3D модели", &["obj", "gltf", "glb"])
            .pick_file()
        else {
            return;
        };
        let path_str = path.to_string_lossy().into_owned();
        if let Some(p) = self.active_project_mut() {
            if let Some(id) = p.active_part_id.clone() {
                if let Some(part) = p.parts.iter_mut().find(|x| x.id == id) {
                    part.model_path = Some(path_str.clone());
                }
            }
        }
        self.touch();
        self.status = format!("Модель: {path_str}");
    }

    pub fn open_model_library_picker(&mut self) {
        let role = self
            .active_part()
            .map(|p| p.role)
            .unwrap_or(PartRole::Other);
        self.library_picker = Some(LibraryPicker::Models { role });
    }

    pub fn open_texture_library_picker(&mut self) {
        self.library_picker = Some(LibraryPicker::Textures {
            category_id: self.texture_category_filter.clone(),
            target: TextureTarget::ActivePart,
        });
    }

    pub fn open_plot_texture_picker(&mut self) {
        self.library_picker = Some(LibraryPicker::Textures {
            category_id: self.texture_category_filter.clone(),
            target: TextureTarget::Plot,
        });
    }

    pub fn apply_model_from_library(&mut self, catalog_id: &str) {
        let item = self
            .library
            .items
            .iter()
            .find(|c| c.id == catalog_id)
            .cloned();
        let Some(item) = item else {
            return;
        };
        if let Some(p) = self.active_project_mut() {
            if let Some(id) = p.active_part_id.clone() {
                if let Some(part) = p.parts.iter_mut().find(|x| x.id == id) {
                    part.model_path = Some(item.file_path.clone());
                    if part.name == part.role.label() {
                        part.name = item.name.clone();
                    }
                }
            }
        }
        self.library_picker = None;
        self.touch();
        self.status = format!("Из библиотеки: {}", item.name);
    }

    pub fn apply_texture_from_library(&mut self, texture_id: &str, target: TextureTarget) {
        let item = self
            .library
            .textures
            .iter()
            .find(|t| t.id == texture_id)
            .cloned();
        let Some(item) = item else {
            return;
        };
        match target {
            TextureTarget::ActivePart => {
                if let Some(p) = self.active_project_mut() {
                    if let Some(id) = p.active_part_id.clone() {
                        if let Some(part) = p.parts.iter_mut().find(|x| x.id == id) {
                            part.texture_path = Some(item.file_path.clone());
                        }
                    }
                }
                self.status = format!("Текстура детали: {}", item.name);
            }
            TextureTarget::Plot => {
                if let Some(p) = self.active_project_mut() {
                    p.plot_texture_path = Some(item.file_path.clone());
                    p.updated_at = chrono::Utc::now();
                }
                self.status = format!("Текстура участка: {}", item.name);
            }
        }
        self.library_picker = None;
        self.rebuild_needed = true;
        self.persist();
    }

    pub fn clear_part_texture(&mut self) {
        if let Some(p) = self.active_project_mut() {
            if let Some(id) = p.active_part_id.clone() {
                if let Some(part) = p.parts.iter_mut().find(|x| x.id == id) {
                    part.texture_path = None;
                }
            }
        }
        self.touch();
        self.status = "Текстура детали снята".into();
    }

    pub fn clear_plot_texture(&mut self) {
        if let Some(p) = self.active_project_mut() {
            p.plot_texture_path = None;
            p.updated_at = chrono::Utc::now();
        }
        self.rebuild_needed = true;
        self.persist();
        self.status = "Текстура участка снята".into();
    }

    pub fn import_to_library(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Добавить модель в библиотеку")
            .add_filter("3D модели", &["obj", "gltf", "glb"])
            .pick_file()
        else {
            return;
        };
        let default_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Модель")
            .to_string();
        let name = if self.new_model_name.trim().is_empty() {
            default_name
        } else {
            self.new_model_name.trim().to_string()
        };
        match library::add_model_from_file(&mut self.library, &path, name, self.catalog_role) {
            Ok(item) => {
                self.preview_cache.invalidate(&item.id);
                self.new_model_name.clear();
                self.status = format!("В библиотеке: {}", item.name);
                self.persist();
            }
            Err(e) => self.status = format!("Ошибка: {e}"),
        }
    }

    pub fn request_delete_library_item(&mut self, id: &str) {
        let name = self
            .library
            .items
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "модель".into());
        self.confirm = Some(ConfirmAction::DeleteLibraryItem {
            id: id.to_string(),
            name,
        });
    }

    pub fn import_texture_to_library(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Добавить текстуру в библиотеку")
            .add_filter("Изображения", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        else {
            return;
        };
        let default_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Текстура")
            .to_string();
        let name = if self.new_texture_name.trim().is_empty() {
            default_name
        } else {
            self.new_texture_name.trim().to_string()
        };
        let category_id = self.texture_category_filter.clone();
        match library::add_texture_from_file(&mut self.library, &path, name, category_id) {
            Ok(item) => {
                self.preview_cache.invalidate(&item.id);
                self.new_texture_name.clear();
                self.status = format!("Текстура в библиотеке: {}", item.name);
                self.persist();
            }
            Err(e) => self.status = format!("Ошибка: {e}"),
        }
    }

    pub fn add_texture_category(&mut self) {
        let name = self.new_texture_category_name.trim().to_string();
        if name.is_empty() {
            self.status = "Укажите название библиотеки текстур".into();
            return;
        }
        let cat = library::add_texture_category(&mut self.library, name);
        self.texture_category_filter = Some(cat.id);
        self.new_texture_category_name.clear();
        self.status = format!("Библиотека текстур: {}", cat.name);
        self.persist();
    }

    pub fn request_delete_texture(&mut self, id: &str) {
        let name = self
            .library
            .textures
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "текстура".into());
        self.confirm = Some(ConfirmAction::DeleteTexture {
            id: id.to_string(),
            name,
        });
    }

    pub fn request_delete_texture_category(&mut self, id: &str) {
        let name = self
            .library
            .texture_categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "библиотека".into());
        self.confirm = Some(ConfirmAction::DeleteTextureCategory {
            id: id.to_string(),
            name,
        });
    }

    pub fn import_font_to_library(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Добавить шрифт в библиотеку")
            .add_filter("Шрифты", &["ttf", "otf"])
            .pick_file()
        else {
            return;
        };
        let default_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Шрифт")
            .to_string();
        let name = if self.new_font_name.trim().is_empty() {
            default_name
        } else {
            self.new_font_name.trim().to_string()
        };
        match library::add_font_from_file(&mut self.library, &path, name) {
            Ok(item) => {
                self.new_font_name.clear();
                self.status = format!("Шрифт в библиотеке: {}", item.name);
                self.persist();
            }
            Err(e) => self.status = format!("Ошибка: {e}"),
        }
    }

    pub fn request_delete_font(&mut self, id: &str) {
        let name = self
            .library
            .fonts
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "шрифт".into());
        self.confirm = Some(ConfirmAction::DeleteFont {
            id: id.to_string(),
            name,
        });
    }

    pub fn open_text_editor_new(&mut self) {
        let Some(part) = self.active_part().cloned() else {
            self.status = "Выберите стелу".into();
            return;
        };
        if part.role != PartRole::Stele {
            self.status = "Гравировка наносится на стелу — выберите стелу".into();
            return;
        }
        let mut editor = TextEditorState::new_for_part(&part.id, &self.draft_text);
        // Prefer first library font
        if let Some(f) = self.library.fonts.first() {
            editor.style.font_id = Some(f.id.clone());
            editor.style.font_path = Some(f.file_path.clone());
        }
        self.text_editor = Some(editor);
    }

    pub fn open_text_editor_edit(&mut self, decal_id: &str) {
        let Some(part) = self.active_part().cloned() else {
            return;
        };
        let Some(decal) = part.decals.iter().find(|d| d.id == decal_id).cloned() else {
            return;
        };
        let DecalKind::Text {
            content,
            style,
            font_scale,
        } = decal.kind
        else {
            self.status = "Слой не является текстом".into();
            return;
        };
        let mut style = style;
        // Migrate legacy relative scale → letter height if still default-ish
        if style.letter_cm <= 0.05 || (style.letter_cm - 4.0).abs() < 0.01 && font_scale != 0.55 {
            let face_h_cm = part.size.height.max(10.0);
            style.letter_cm = (face_h_cm * decal.height * font_scale).clamp(1.0, 20.0);
        }
        if style.font_path.is_none() {
            if let Some(id) = &style.font_id {
                if let Some(f) = self.library.fonts.iter().find(|f| &f.id == id) {
                    style.font_path = Some(f.file_path.clone());
                }
            } else if let Some(f) = self.library.fonts.first() {
                style.font_id = Some(f.id.clone());
                style.font_path = Some(f.file_path.clone());
            }
        }
        self.text_editor = Some(TextEditorState {
            part_id: part.id,
            decal_id: Some(decal.id),
            content,
            style,
            name: decal.name,
            side: decal.side,
        });
    }

    pub fn apply_text_editor(&mut self) {
        let Some(editor) = self.text_editor.take() else {
            return;
        };
        if editor.content.trim().is_empty() {
            self.status = "Введите текст надписи".into();
            self.text_editor = Some(editor);
            return;
        }
        let mut style = editor.style.clone();
        // Resolve font path from library id
        if let Some(id) = &style.font_id {
            if let Some(f) = self.library.fonts.iter().find(|f| &f.id == id) {
                style.font_path = Some(f.file_path.clone());
            }
        }
        let mut new_id = None;
        if let Some(p) = self.active_project_mut() {
            if let Some(part) = p.parts.iter_mut().find(|x| x.id == editor.part_id) {
                if let Some(did) = &editor.decal_id {
                    if let Some(d) = part.decals.iter_mut().find(|d| &d.id == did) {
                        if !editor.name.trim().is_empty() {
                            d.name = editor.name.clone();
                        } else {
                            d.name = editor
                                .content
                                .lines()
                                .next()
                                .unwrap_or("Текст")
                                .chars()
                                .take(24)
                                .collect();
                        }
                        d.kind = DecalKind::Text {
                            content: editor.content.clone(),
                            font_scale: 0.55,
                            style: style.clone(),
                        };
                        d.side = editor.side;
                        new_id = Some(d.id.clone());
                    }
                } else {
                    let mut d = Decal::new_text(editor.content.clone());
                    d.side = editor.side;
                    d.kind = DecalKind::Text {
                        content: editor.content.clone(),
                        font_scale: 0.55,
                        style: style.clone(),
                    };
                    new_id = Some(d.id.clone());
                    part.decals.push(d);
                }
            }
        }
        if let Some(id) = new_id {
            self.active_decal_id = Some(id);
            self.touch();
            self.status = "Гравировка текста применена".into();
        }
    }

    pub fn add_text_decal(&mut self) {
        self.open_text_editor_new();
    }

    pub fn add_photo_decal(&mut self) {
        self.open_photo_editor_new();
    }

    pub fn open_photo_editor_new(&mut self) {
        let Some(part) = self.active_part().cloned() else {
            self.status = "Выберите стелу".into();
            return;
        };
        if part.role != PartRole::Stele {
            self.status = "Фотокерамика наносится на стелу — выберите стелу".into();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Портрет для медальона")
            .add_filter("Изображения", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        let path_str = path.to_string_lossy().into_owned();
        let name = std::path::Path::new(&path_str)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Фото")
            .to_string();
        let mut spec = PhotoSpec::default();
        if let Some(f) = self.library.fonts.first() {
            spec.text.font_id = Some(f.id.clone());
            spec.text.font_path = Some(f.file_path.clone());
        }
        self.photo_editor = Some(PhotoEditorState {
            part_id: part.id,
            decal_id: None,
            path: path_str,
            name,
            side: FaceSide::Front,
            spec,
        });
    }

    pub fn open_photo_editor_edit(&mut self, decal_id: &str) {
        let Some(part) = self.active_part().cloned() else {
            return;
        };
        let Some(decal) = part.decals.iter().find(|d| d.id == decal_id).cloned() else {
            return;
        };
        let DecalKind::Photo { path, spec } = decal.kind else {
            self.status = "Слой не является фотокерамикой".into();
            return;
        };
        self.photo_editor = Some(PhotoEditorState {
            part_id: part.id,
            decal_id: Some(decal.id),
            path,
            name: decal.name,
            side: decal.side,
            spec,
        });
    }

    pub fn apply_photo_editor(&mut self) {
        let Some(editor) = self.photo_editor.take() else {
            return;
        };
        if editor.path.trim().is_empty() {
            self.status = "Выберите фото".into();
            self.photo_editor = Some(editor);
            return;
        }
        let mut spec = editor.spec.clone();
        if let Some(id) = &spec.background_id {
            if let Some(bg) = self.library.backgrounds.iter().find(|b| &b.id == id) {
                spec.background_path = Some(bg.file_path.clone());
            }
        } else {
            spec.background_path = None;
        }
        if let Some(id) = &spec.text.font_id {
            if let Some(f) = self.library.fonts.iter().find(|f| &f.id == id) {
                spec.text.font_path = Some(f.file_path.clone());
            }
        }
        let mut new_id = None;
        if let Some(p) = self.active_project_mut() {
            if let Some(part) = p.parts.iter_mut().find(|x| x.id == editor.part_id) {
                if let Some(did) = &editor.decal_id {
                    if let Some(d) = part.decals.iter_mut().find(|d| &d.id == did) {
                        if !editor.name.trim().is_empty() {
                            d.name = editor.name.clone();
                        }
                        d.side = editor.side;
                        d.kind = DecalKind::Photo {
                            path: editor.path.clone(),
                            spec: spec.clone(),
                        };
                        new_id = Some(d.id.clone());
                    }
                } else {
                    let mut d = Decal::new_photo(editor.path.clone());
                    d.side = editor.side;
                    if !editor.name.trim().is_empty() {
                        d.name = editor.name.clone();
                    }
                    d.kind = DecalKind::Photo {
                        path: editor.path.clone(),
                        spec,
                    };
                    new_id = Some(d.id.clone());
                    part.decals.push(d);
                }
            }
        }
        if let Some(id) = new_id {
            self.active_decal_id = Some(id);
            self.textures.clear();
            self.touch();
            self.status = "Фотокерамика применена".into();
        }
    }

    pub fn import_background_to_library(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Фон медальона")
            .add_filter("Изображения", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        let default_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Фон")
            .to_string();
        let name = if self.new_background_name.trim().is_empty() {
            default_name
        } else {
            self.new_background_name.trim().to_string()
        };
        let shape = self.background_shape;
        match library::add_background_from_file(&mut self.library, &path, name, shape) {
            Ok(item) => {
                self.new_background_name.clear();
                self.status = format!(
                    "Фон «{}» в библиотеке «{}»",
                    item.name,
                    item.shape.label()
                );
                self.persist();
            }
            Err(e) => self.status = format!("Ошибка: {e}"),
        }
    }

    pub fn request_delete_background(&mut self, id: &str) {
        let name = self
            .library
            .backgrounds
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "фон".into());
        self.confirm = Some(ConfirmAction::DeleteBackground {
            id: id.to_string(),
            name,
        });
    }

    pub fn move_active_decal(&mut self, delta: i32) {
        let part_id = self
            .active_project()
            .and_then(|p| p.active_part_id.clone());
        let decal_id = self.active_decal_id.clone();
        let (Some(part_id), Some(decal_id)) = (part_id, decal_id) else {
            return;
        };
        if let Some(p) = self.active_project_mut() {
            if let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) {
                if let Some(idx) = part.decals.iter().position(|d| d.id == decal_id) {
                    let new_idx = (idx as i32 + delta).clamp(0, part.decals.len() as i32 - 1) as usize;
                    if new_idx != idx {
                        part.decals.swap(idx, new_idx);
                        p.updated_at = chrono::Utc::now();
                        self.rebuild_needed = true;
                        self.persist();
                    }
                }
            }
        }
    }

    pub fn delete_active_decal(&mut self) {
        let part_id = self
            .active_project()
            .and_then(|p| p.active_part_id.clone());
        let decal_id = self.active_decal_id.clone();
        let (Some(part_id), Some(decal_id)) = (part_id, decal_id) else {
            return;
        };
        if let Some(p) = self.active_project_mut() {
            if let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) {
                part.decals.retain(|d| d.id != decal_id);
                self.active_decal_id = part.decals.last().map(|d| d.id.clone());
            }
        }
        self.touch();
        self.status = "Слой эскиза удалён".into();
    }

    pub fn apply_confirm(&mut self) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::DeletePart { id, name } => {
                if let Some(p) = self.active_project_mut() {
                    p.parts.retain(|x| x.id != id);
                    p.active_part_id = p.parts.first().map(|x| x.id.clone());
                }
                self.frame_camera = true;
                self.touch();
                self.status = format!("Удалено: {name}");
            }
            ConfirmAction::DeleteMonument { id, name } => {
                if let Some(p) = self.active_project_mut() {
                    if p.monuments.len() <= 1 {
                        self.status = "Нельзя удалить единственный памятник".into();
                        return;
                    }
                    p.parts.retain(|x| x.monument_id != id);
                    p.monuments.retain(|m| m.id != id);
                    p.active_monument_id = p.monuments.first().map(|m| m.id.clone());
                    let mid = p.active_monument_id.clone();
                    p.active_part_id = p
                        .parts
                        .iter()
                        .find(|x| Some(&x.monument_id) == mid.as_ref())
                        .map(|x| x.id.clone())
                        .or_else(|| p.parts.first().map(|x| x.id.clone()));
                }
                self.frame_camera = true;
                self.touch();
                self.status = format!("Удалён памятник: {name}");
            }
            ConfirmAction::DeleteLibraryItem { id, name } => {
                match library::remove_model(&mut self.library, &id) {
                    Ok(()) => {
                        self.preview_cache.invalidate(&id);
                        self.status = format!("Удалено из библиотеки: {name}");
                        self.persist();
                    }
                    Err(e) => self.status = format!("Ошибка: {e}"),
                }
            }
            ConfirmAction::DeleteTexture { id, name } => {
                match library::remove_texture(&mut self.library, &id) {
                    Ok(()) => {
                        self.preview_cache.invalidate(&id);
                        self.status = format!("Текстура удалена: {name}");
                        self.persist();
                    }
                    Err(e) => self.status = format!("Ошибка: {e}"),
                }
            }
            ConfirmAction::DeleteTextureCategory { id, name } => {
                match library::remove_texture_category(&mut self.library, &id) {
                    Ok(()) => {
                        if self.texture_category_filter.as_deref() == Some(id.as_str()) {
                            self.texture_category_filter = None;
                        }
                        self.status = format!("Библиотека текстур удалена: {name}");
                        self.persist();
                    }
                    Err(e) => self.status = format!("Ошибка: {e}"),
                }
            }
            ConfirmAction::DeleteFont { id, name } => {
                match library::remove_font(&mut self.library, &id) {
                    Ok(()) => {
                        self.status = format!("Шрифт удалён: {name}");
                        self.persist();
                    }
                    Err(e) => self.status = format!("Ошибка: {e}"),
                }
            }
            ConfirmAction::DeleteBackground { id, name } => {
                match library::remove_background(&mut self.library, &id) {
                    Ok(()) => {
                        self.preview_cache.invalidate(&id);
                        self.status = format!("Фон удалён: {name}");
                        self.persist();
                    }
                    Err(e) => self.status = format!("Ошибка: {e}"),
                }
            }
            ConfirmAction::DeleteProject { id, name } => {
                self.data.projects.retain(|p| p.id != id);
                if self.data.active_project_id.as_deref() == Some(id.as_str()) {
                    self.data.active_project_id = None;
                }
                self.persist();
                self.status = format!("Удалён проект: {name}");
            }
        }
    }

    pub fn show_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm.clone() else {
            return;
        };
        let (title, body) = match &action {
            ConfirmAction::DeletePart { name, .. } => (
                "Удалить деталь?",
                format!("Удалить «{name}» с участка? Это действие нельзя отменить."),
            ),
            ConfirmAction::DeleteMonument { name, .. } => (
                "Удалить памятник?",
                format!(
                    "Удалить «{name}» и все его детали? Это действие нельзя отменить."
                ),
            ),
            ConfirmAction::DeleteLibraryItem { name, .. } => (
                "Удалить из библиотеки?",
                format!("Удалить модель «{name}» из библиотеки и с диска?"),
            ),
            ConfirmAction::DeleteTexture { name, .. } => (
                "Удалить текстуру?",
                format!("Удалить текстуру «{name}» из библиотеки и с диска?"),
            ),
            ConfirmAction::DeleteTextureCategory { name, .. } => (
                "Удалить библиотеку текстур?",
                format!(
                    "Удалить библиотеку «{name}»? Текстуры останутся без категории."
                ),
            ),
            ConfirmAction::DeleteFont { name, .. } => (
                "Удалить шрифт?",
                format!("Удалить шрифт «{name}» из библиотеки и с диска?"),
            ),
            ConfirmAction::DeleteBackground { name, .. } => (
                "Удалить фон?",
                format!("Удалить фон «{name}» из библиотеки и с диска?"),
            ),
            ConfirmAction::DeleteProject { name, .. } => (
                "Удалить проект?",
                format!("Удалить проект «{name}» из списка?"),
            ),
        };

        let mut open = true;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(&body);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Удалить")
                                    .color(egui::Color32::from_rgb(255, 230, 230)),
                            )
                            .fill(egui::Color32::from_rgb(140, 45, 40)),
                        )
                        .clicked()
                    {
                        self.apply_confirm();
                    }
                    if ui.button("Отмена").clicked() {
                        self.confirm = None;
                    }
                });
                ui.label(
                    egui::RichText::new("Enter — подтвердить · Esc — отмена")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            });

        if !open {
            self.confirm = None;
        }

        if self.confirm.is_some() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.apply_confirm();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm = None;
            }
        }
    }
}

impl eframe::App for StelaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::apply_theme(ctx);
        if self.rebuild_needed {
            self.rebuild_scene();
        }

        // Delete hotkey (not while typing)
        let delete_pressed = ctx.input(|i| {
            !i.modifiers.any()
                && (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        });
        if delete_pressed && self.confirm.is_none() && !ctx.wants_keyboard_input() {
            if self.screen == Screen::Studio {
                self.request_delete_on_hotkey();
            }
        }

        match self.screen {
            Screen::Home => ui::home::show(self, ctx),
            Screen::Studio => ui::studio::show(self, ctx),
            Screen::Library => ui::library::show(self, ctx),
        }

        ui::studio::show_library_picker(self, ctx);
        ui::text_editor::show(self, ctx);
        ui::photo_editor::show(self, ctx);
        self.show_confirm_modal(ctx);
    }
}
