//! Écran d'accueil : la bibliothèque (maquette `Main`). Barre latérale (recherche, navigation,
//! matières), puis « Reprendre », les pages récentes et les couvertures des cahiers ; et la feuille
//! de création ou de modification d'un cahier ou d'une matière (maquette `NouveauCahier`).

use std::collections::HashMap;

use calame_core::background::PATTERN_BACKGROUNDS;
use calame_core::{Background, Color, LibraryTree, Node, NodeId, NodeKind};
use egui::epaint::TextShape;
use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, Layout, Painter, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2,
};

use crate::dates;
use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::library_ui::{node_color, page_order, Action};
use crate::theme::{self, Weight};
use crate::thumbs::{self, ThumbSource};
use crate::toolbar::{background_key, Command};
use crate::widgets::{self, Button, ButtonStyle};

/// Rubrique de la barre latérale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Nav {
    #[default]
    Home,
    Recent,
    Favorites,
    Trash,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SheetTarget {
    Create,
    Edit(NodeId),
}

/// Feuille de création ou de modification d'un cahier ou d'une matière.
pub struct Sheet {
    target: SheetTarget,
    kind: NodeKind,
    name: String,
    color: Color,
    icon: String,
    /// Matière du cahier ; `None` : une nouvelle matière, nommée `new_subject`.
    subject: Option<NodeId>,
    new_subject: String,
    paper: Background,
    /// Vrai avant la première image, pour donner le focus au champ du nom.
    fresh: bool,
}

#[derive(Default)]
pub struct HomeState {
    pub nav: Nav,
    /// Matière choisie dans les puces de « Vos cahiers ».
    filter: Option<NodeId>,
    pub sheet: Option<Sheet>,
}

impl HomeState {
    /// Matière choisie dans la bibliothèque (destination d'un PDF importé).
    pub fn filter(&self) -> Option<NodeId> {
        self.filter
    }

    /// Ouvre la feuille « Nouveau cahier », dans la matière choisie si elle est donnée.
    pub fn open_new_notebook(&mut self, tree: &LibraryTree, subject: Option<NodeId>) {
        let subjects = tree.children(None);
        let subject = subject
            .or(self.filter)
            .filter(|s| tree.get(*s).is_some_and(|n| n.kind == NodeKind::Subject))
            .or_else(|| subjects.first().map(|s| s.id));
        let color = subject
            .and_then(|s| tree.get(s))
            .map_or(theme::COVER_COLORS[0], |s| s.color);
        self.sheet = Some(Sheet {
            target: SheetTarget::Create,
            kind: NodeKind::Notebook,
            name: String::new(),
            color,
            icon: NodeKind::Notebook.default_icon().to_owned(),
            subject,
            new_subject: t("welcome-subject").to_owned(),
            paper: Background::GridSmall,
            fresh: true,
        });
    }

    pub fn open_new_subject(&mut self, tree: &LibraryTree) {
        let index = tree.children(None).len();
        self.sheet = Some(Sheet {
            target: SheetTarget::Create,
            kind: NodeKind::Subject,
            name: String::new(),
            color: theme::COVER_COLORS[index % theme::COVER_COLORS.len()],
            icon: Icon::Book.key().to_owned(),
            subject: None,
            new_subject: String::new(),
            paper: Background::Blank,
            fresh: true,
        });
    }

    fn open_edit(&mut self, tree: &LibraryTree, node: &Node, paper: Background) {
        self.sheet = Some(Sheet {
            target: SheetTarget::Edit(node.id),
            kind: node.kind,
            name: node.name.clone(),
            color: node_color(tree, node),
            icon: node.icon.clone(),
            subject: node.parent_id,
            new_subject: String::new(),
            paper,
            fresh: true,
        });
    }
}

/// Ce que l'écran d'accueil lit de l'application.
pub struct HomeData<'a> {
    pub tree: &'a LibraryTree,
    pub trash: &'a [Node],
    /// Date de dernière modification de chaque page (secondes Unix).
    pub stamps: &'a HashMap<NodeId, i64>,
    pub favorites: &'a [NodeId],
    /// Papier des nouvelles pages de chaque cahier.
    pub papers: &'a HashMap<NodeId, Background>,
    pub dark: bool,
    /// Des fichiers sont tirés au-dessus de la fenêtre.
    pub dragging_files: bool,
}

fn count_label(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Encre lisible sur une couleur de couverture (contraste d'au moins 4,5:1).
fn on_color(color: Color32) -> Color32 {
    theme::on_color(color)
}

// ---- Couvertures ------------------------------------------------------------------------

/// Peintre d'une couverture inclinée de `angle` radians autour de son centre.
struct Tilted<'a> {
    painter: &'a Painter,
    center: Pos2,
    rot: egui::emath::Rot2,
}

impl Tilted<'_> {
    fn point(&self, p: Pos2) -> Pos2 {
        self.center + self.rot * (p - self.center)
    }

    fn fill(&self, rect: Rect, radius: CornerRadius, color: Color32) {
        let pts = widgets::rounded_rect_points(rect, radius)
            .into_iter()
            .map(|p| self.point(p))
            .collect();
        self.painter
            .add(Shape::convex_polygon(pts, color, Stroke::NONE));
    }

    fn text(&self, pos: Pos2, galley: std::sync::Arc<egui::Galley>, color: Color32) {
        let angle = self.rot.angle();
        self.painter
            .add(TextShape::new(self.point(pos), galley, color).with_angle(angle));
    }
}

/// Couverture : dos ombré de 12 px à gauche, matière en petites capitales, titre en bas.
fn draw_cover(
    painter: &Painter,
    cover: Rect,
    color: Color32,
    subject: &str,
    title: &str,
    angle: f32,
) {
    let radius = theme::cover_radius(1.0);
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 12],
            blur: 22,
            spread: 0,
            color: Color32::from_black_alpha(60),
        }
        .as_shape(cover.shrink2(vec2(6.0, 4.0)), radius),
    );
    let tilted = Tilted {
        painter,
        center: cover.center(),
        rot: egui::emath::Rot2::from_angle(angle),
    };
    tilted.fill(cover, radius, color);
    let spine = Rect::from_min_max(cover.min, pos2(cover.left() + 12.0, cover.bottom()));
    tilted.fill(
        spine,
        CornerRadius {
            nw: radius.nw,
            sw: radius.sw,
            ne: 0,
            se: 0,
        },
        Color32::from_black_alpha(46),
    );
    painter.line_segment(
        [
            tilted.point(pos2(spine.right(), cover.top())),
            tilted.point(pos2(spine.right(), cover.bottom())),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(64)),
    );
    let ink = on_color(color);
    let left = cover.left() + 26.0;
    let width = cover.right() - left - 14.0;
    if !subject.is_empty() {
        let mut job = widgets::caps_job(subject, ink.gamma_multiply(0.85), 11.0);
        job.wrap = egui::text::TextWrapping::truncate_at_width(width);
        let galley = painter.layout_job(job);
        tilted.text(pos2(left, cover.top() + 16.0), galley, ink);
    }
    let mut job = egui::text::LayoutJob::simple(title.to_owned(), theme::display(19.0), ink, width);
    job.wrap.max_rows = 4;
    let galley = painter.layout_job(job);
    let pos = pos2(left, cover.bottom() - 14.0 - galley.size().y);
    tilted.text(pos, galley, ink);
}

// ---- Écran ------------------------------------------------------------------------------

pub fn show(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let dark = data.dark;
    if state.filter.is_some_and(|f| data.tree.get(f).is_none()) {
        state.filter = None;
    }
    egui::Panel::left("bibliotheque-laterale")
        .exact_size(264.0)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(theme::sidebar(dark))
                .stroke(Stroke::new(1.0, theme::border(dark)))
                .inner_margin(egui::Margin::symmetric(14, 20)),
        )
        .show(ui, |ui| sidebar(ui, state, data, &mut actions));
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::app_background(dark)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let side = (ui.available_width() * 0.04).clamp(20.0, 56.0) as i8;
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: side,
                            right: side,
                            top: 36,
                            bottom: 56,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            match state.nav {
                                Nav::Home => home(ui, state, data, thumbs, &mut actions),
                                Nav::Recent => recent_view(ui, data, thumbs, &mut actions),
                                Nav::Favorites => favorites_view(ui, state, data, &mut actions),
                                Nav::Trash => trash_view(ui, data, &mut actions),
                            }
                        });
                });
        });
    actions
}

fn nav_item(
    ui: &mut Ui,
    icon: Icon,
    text: &str,
    active: bool,
    badge: Option<String>,
) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let (fill, color, weight) = if active {
            (
                theme::accent_soft(dark),
                theme::accent(dark),
                Weight::Semibold,
            )
        } else if response.hovered() {
            (theme::hover(dark), theme::icon(dark), Weight::Regular)
        } else {
            (Color32::TRANSPARENT, theme::icon(dark), Weight::Regular)
        };
        ui.painter().rect_filled(rect, theme::RADIUS_BUTTON, fill);
        let glyph =
            Rect::from_center_size(pos2(rect.left() + 21.0, rect.center().y), Vec2::splat(18.0));
        icons::paint(ui.painter(), glyph, icon, color);
        ui.painter().text(
            pos2(glyph.right() + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            theme::font(14.0, weight),
            color,
        );
        if let Some(badge) = badge {
            ui.painter().text(
                pos2(rect.right() - 12.0, rect.center().y),
                Align2::RIGHT_CENTER,
                badge,
                theme::font(12.0, Weight::Regular),
                theme::label_secondary(dark),
            );
        }
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, text));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn sidebar(ui: &mut Ui, state: &mut HomeState, data: &HomeData<'_>, actions: &mut Vec<Action>) {
    let dark = data.dark;
    egui::Panel::bottom("bibliotheque-reglages")
        .resizable(false)
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 12,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, theme::border(dark));
            ui.add_space(10.0);
            let settings = nav_item(ui, Icon::Settings, t("settings"), false, None);
            egui::Popup::menu(&settings).show(|ui| {
                ui.set_min_width(240.0);
                for (key, command) in [
                    ("import-pdf", Command::ImportPdf),
                    ("backup", Command::Backup),
                    ("restore-backup", Command::RestoreBackup),
                ] {
                    if ui.button(t(key)).clicked() {
                        actions.push(Action::Command(command));
                        ui.close();
                    }
                }
            });
            let (icon, key) = if dark {
                (Icon::Sun, "light-theme")
            } else {
                (Icon::Moon, "dark-theme")
            };
            if nav_item(ui, icon, t(key), false, None).clicked() {
                actions.push(Action::Command(Command::SetDark(!dark)));
            }
        });

    ui.spacing_mut().item_spacing.y = 2.0;
    // Logo et nom.
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
        icons::logo(ui.painter(), rect, theme::accent(dark));
        ui.add_space(2.0);
        ui.label(
            RichText::new("Calame")
                .font(theme::display(21.0))
                .color(theme::label(dark)),
        );
    });
    ui.add_space(20.0);

    // Recherche : ouvre la palette.
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    let fill = if response.hovered() {
        theme::hover(dark)
    } else {
        theme::surface(dark)
    };
    ui.painter().rect(
        rect,
        theme::RADIUS_BUTTON,
        fill,
        Stroke::new(1.0, theme::border(dark)),
        StrokeKind::Inside,
    );
    let glyph =
        Rect::from_center_size(pos2(rect.left() + 20.5, rect.center().y), Vec2::splat(17.0));
    icons::paint(
        ui.painter(),
        glyph,
        Icon::Search,
        theme::label_secondary(dark),
    );
    ui.painter().text(
        pos2(glyph.right() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        t("search"),
        theme::font(14.0, Weight::Regular),
        theme::label_secondary(dark),
    );
    widgets::kbd(
        ui.painter(),
        pos2(rect.right() - 10.0, rect.center().y),
        "Ctrl K",
        dark,
        false,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("search")));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        actions.push(Action::Command(Command::OpenPalette));
    }
    ui.add_space(22.0);

    let trash_badge = (!data.trash.is_empty()).then(|| data.trash.len().to_string());
    for (nav, icon, key, badge) in [
        (Nav::Home, Icon::Home, "home", None),
        (Nav::Recent, Icon::Clock, "recent", None),
        (Nav::Favorites, Icon::Favorite, "favorites", None),
        (Nav::Trash, Icon::Trash, "trash", trash_badge),
    ] {
        let active = state.nav == nav && !(nav == Nav::Home && state.filter.is_some());
        if nav_item(ui, icon, t(key), active, badge).clicked() {
            state.nav = nav;
            state.filter = None;
        }
    }
    ui.add_space(22.0);

    // Matières.
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        widgets::section_caps(ui, t("subjects"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button_sized(ui, Icon::Plus, t("new-subject"), false, true, 28.0)
                .clicked()
            {
                state.open_new_subject(data.tree);
            }
        });
    });
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for subject in data.tree.children(None) {
                subject_row(ui, state, data, subject, actions);
            }
        });
}

fn subject_row(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    subject: &Node,
    actions: &mut Vec<Action>,
) {
    let dark = data.dark;
    let active = state.nav == Nav::Home && state.filter == Some(subject.id);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if active {
            theme::accent_soft(dark)
        } else if response.hovered() {
            theme::hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, theme::RADIUS_BUTTON, fill);
        let square =
            Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(10.0));
        ui.painter()
            .rect_filled(square, 3.0, theme::to_color32(subject.color));
        let count = data.tree.children(Some(subject.id)).len().to_string();
        let count_rect = ui.painter().text(
            pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            count,
            theme::font(12.0, Weight::Regular),
            theme::label_secondary(dark),
        );
        let x = square.right() + 12.0;
        widgets::truncated(
            ui.painter(),
            pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            &subject.name,
            theme::font(
                14.0,
                if active {
                    Weight::Semibold
                } else {
                    Weight::Regular
                },
            ),
            if active {
                theme::accent(dark)
            } else {
                theme::icon(dark)
            },
            count_rect.left() - x - 8.0,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, active, &subject.name)
    });
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        state.nav = Nav::Home;
        state.filter = if active { None } else { Some(subject.id) };
    }
    response.context_menu(|ui| node_menu(ui, state, data, subject, actions));
}

// ---- Accueil ----------------------------------------------------------------------------

fn header(ui: &mut Ui, data: &HomeData<'_>, actions: &mut Vec<Action>, state: &mut HomeState) {
    let dark = data.dark;
    let now = dates::now();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(
                RichText::new(dates::long_day(now))
                    .font(theme::font(14.0, Weight::Regular))
                    .color(theme::label_secondary(dark)),
            );
            ui.label(
                RichText::new(dates::greeting(now))
                    .font(theme::display(40.0))
                    .color(theme::label(dark)),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if Button::new(t("new-notebook"), ButtonStyle::Primary)
                .icon(Icon::Plus)
                .show(ui)
                .clicked()
            {
                state.open_new_notebook(data.tree, None);
            }
            if Button::new(t("import-pdf-short"), ButtonStyle::Secondary)
                .icon(Icon::ImportPdf)
                .show(ui)
                .clicked()
            {
                actions.push(Action::Command(Command::ImportPdf));
            }
        });
    });
}

fn section_title(ui: &mut Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new(text)
            .font(theme::display(22.0))
            .color(theme::label(dark)),
    );
}

/// Pages modifiées, de la plus récente à la plus ancienne.
fn recent_pages(data: &HomeData<'_>, limit: usize) -> Vec<(NodeId, i64)> {
    let mut pages: Vec<(NodeId, i64)> = data
        .stamps
        .iter()
        .filter(|(id, _)| {
            data.tree
                .get(**id)
                .is_some_and(|n| n.kind == NodeKind::Page)
        })
        .map(|(id, at)| (*id, *at))
        .collect();
    pages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    pages.truncate(limit);
    pages
}

fn home(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
    actions: &mut Vec<Action>,
) {
    header(ui, data, actions, state);
    ui.add_space(40.0);
    resume_row(ui, state, data, thumbs, actions);
    let recents = recent_pages(data, 6);
    if !recents.is_empty() {
        ui.add_space(40.0);
        ui.horizontal(|ui| {
            section_title(ui, t("recent-pages"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if Button::new(t("see-all"), ButtonStyle::Link)
                    .show(ui)
                    .clicked()
                {
                    state.nav = Nav::Recent;
                }
            });
        });
        ui.add_space(16.0);
        let width = ui.available_width();
        let columns = ((width + 18.0) / (176.0 + 18.0)).floor().max(2.0) as usize;
        let shown: Vec<NodeId> = recents
            .iter()
            .take(columns.min(6))
            .map(|(id, _)| *id)
            .collect();
        page_grid(ui, data, thumbs, &shown, columns, actions);
    }
    ui.add_space(40.0);
    notebooks_section(ui, state, data, actions);
}

/// Grande carte « Reprendre », note rapide et zone de dépôt de PDF.
fn resume_row(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
    actions: &mut Vec<Action>,
) {
    let width = ui.available_width();
    let gap = 20.0;
    let height = 284.0;
    let wide = width >= 820.0;
    let big = if wide {
        (width - gap) * 2.0 / 3.0
    } else {
        width
    };
    let last = recent_pages(data, 1).first().copied();
    let draw = |ui: &mut Ui,
                state: &mut HomeState,
                thumbs: &mut ThumbSource<'_>,
                actions: &mut Vec<Action>| {
        let (rect, _) = ui.allocate_exact_size(vec2(big, height), Sense::hover());
        match last {
            Some((page, at)) => resume_card(ui, rect, data, thumbs, page, at, actions),
            None => welcome_card(ui, rect, state, data),
        }
    };
    let side = |ui: &mut Ui, w: f32, actions: &mut Vec<Action>| {
        let each = (height - gap) / 2.0;
        let (note, _) = ui.allocate_exact_size(vec2(w, each), Sense::hover());
        quick_note_card(ui, note, actions);
        ui.add_space(gap);
        let (drop, _) = ui.allocate_exact_size(vec2(w, each), Sense::hover());
        drop_zone(ui, drop, data, actions);
    };
    if wide {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            draw(ui, state, thumbs, actions);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                side(ui, width - big - gap, actions);
            });
        });
    } else {
        draw(ui, state, thumbs, actions);
        ui.add_space(gap);
        side(ui, width, actions);
    }
}

fn card_frame(painter: &Painter, rect: Rect, dark: bool) {
    painter.add(theme::card_shadow(dark).as_shape(rect, theme::RADIUS_CARD));
    painter.rect(
        rect,
        theme::RADIUS_CARD,
        theme::surface(dark),
        Stroke::new(1.0, theme::border(dark)),
        StrokeKind::Inside,
    );
}

/// Bouton sombre peint (« Continuer à écrire → ») ; la carte entière est cliquable.
fn paint_ink_button(painter: &Painter, min: Pos2, text: &str, hovered: bool, dark: bool) -> Rect {
    let (fill, color) = theme::ink_button(dark);
    let galley =
        painter.layout_no_wrap(text.to_owned(), theme::font(14.0, Weight::Semibold), color);
    let rect = Rect::from_min_size(min, vec2(galley.size().x + 18.0 + 10.0 + 36.0, 44.0));
    painter.rect_filled(
        rect,
        12.0,
        if hovered {
            fill.gamma_multiply(0.86)
        } else {
            fill
        },
    );
    painter.galley(
        pos2(rect.left() + 18.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    icons::paint(
        painter,
        Rect::from_center_size(
            pos2(rect.right() - 26.0, rect.center().y),
            Vec2::splat(16.0),
        ),
        Icon::ArrowRight,
        color,
    );
    rect
}

fn resume_card(
    ui: &mut Ui,
    rect: Rect,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
    page: NodeId,
    at: i64,
    actions: &mut Vec<Action>,
) {
    let dark = data.dark;
    let tree = data.tree;
    let response = ui.interact(rect, ui.scope_id().with("reprendre"), Sense::click());
    let painter = ui.painter();
    card_frame(painter, rect, dark);
    let inner = rect.shrink(22.0);
    let thumb_width = ((inner.width() - 28.0) * 300.0 / 520.0).max(160.0);
    let thumb = Rect::from_min_size(inner.min, vec2(thumb_width, 240.0));
    thumbs.paint(ui.ctx(), painter, thumb, page, 12.0, dark);
    painter.rect_stroke(
        thumb,
        12.0,
        Stroke::new(1.0, theme::border(dark)),
        StrokeKind::Inside,
    );

    let x = thumb.right() + 28.0;
    let text_width = inner.right() - x;
    let mut y = inner.top() + 6.0;
    // Pastille « Reprendre là où vous étiez ».
    let pill_text = t("resume-pill");
    let galley = painter.layout_no_wrap(
        pill_text.to_owned(),
        theme::font(12.0, Weight::Semibold),
        theme::accent(dark),
    );
    let pill = Rect::from_min_size(pos2(x, y), vec2(galley.size().x + 33.0, 26.0));
    painter.rect_filled(pill, 13.0, theme::accent_soft(dark));
    painter.circle_filled(
        pos2(pill.left() + 13.5, pill.center().y),
        3.5,
        theme::accent(dark),
    );
    painter.galley(
        pos2(pill.left() + 23.0, pill.center().y - galley.size().y / 2.0),
        galley,
        theme::accent(dark),
    );
    y = pill.bottom() + 10.0;

    let notebook = tree.notebook_of(page);
    let notebook_name = notebook
        .and_then(|n| tree.get(n))
        .map_or("", |n| n.name.as_str());
    let title = painter.layout(
        notebook_name.to_owned(),
        theme::display(28.0),
        theme::label(dark),
        text_width,
    );
    let title_height = title.size().y;
    painter.galley(pos2(x, y), title, theme::label(dark));
    y += title_height + 10.0;

    let path = tree.path(page);
    let mut parts: Vec<&str> = path
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Subject | NodeKind::Folder))
        .map(|n| n.name.as_str())
        .collect();
    let where_line = if let Some(nb) = notebook {
        let order = page_order(tree, nb);
        let n = order.iter().position(|p| *p == page).map_or(0, |i| i + 1);
        let pages = format!("{} {n} {} {}", t("page"), t("of"), order.len());
        parts.push(&pages);
        parts.join(" · ")
    } else {
        parts.join(" · ")
    };
    let modified = dates::local(at)
        .map(|then| format!("{} {}", t("modified"), dates::relative(dates::now(), then)));
    let secondary = theme::label_secondary(dark);
    for line in [Some(where_line), modified].into_iter().flatten() {
        let galley = painter.layout(
            line,
            theme::font(15.0, Weight::Regular),
            secondary,
            text_width,
        );
        let h = galley.size().y;
        painter.galley(pos2(x, y), galley, secondary);
        y += h + 2.0;
    }
    paint_ink_button(
        painter,
        pos2(x, inner.bottom() - 50.0),
        t("continue-writing"),
        response.hovered(),
        dark,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("continue-writing")));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        actions.push(Action::OpenPage(page));
    }
}

/// Carte de bienvenue, tant qu'aucune page n'a été écrite.
fn welcome_card(ui: &mut Ui, rect: Rect, state: &mut HomeState, data: &HomeData<'_>) {
    let dark = data.dark;
    card_frame(ui.painter(), rect, dark);
    let inner = rect.shrink(32.0);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    child.spacing_mut().item_spacing.y = 10.0;
    let (logo, _) = child.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
    icons::logo(child.painter(), logo, theme::accent(dark));
    child.label(
        RichText::new(t("welcome-title"))
            .font(theme::display(28.0))
            .color(theme::label(dark)),
    );
    child.label(
        RichText::new(t("welcome-text"))
            .font(theme::font(15.0, Weight::Regular))
            .color(theme::label_secondary(dark)),
    );
    child.add_space(6.0);
    if Button::new(t("create-first-notebook"), ButtonStyle::Primary)
        .icon(Icon::Plus)
        .show(&mut child)
        .clicked()
    {
        state.open_new_notebook(data.tree, None);
    }
}

fn quick_note_card(ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
    let response = ui.interact(rect, ui.scope_id().with("note-rapide"), Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() {
        Color32::from_rgb(0x22, 0x25, 0x2D)
    } else {
        theme::INK_BUTTON
    };
    painter.rect_filled(rect, theme::RADIUS_CARD, fill);
    let inner = rect.shrink(20.0);
    let badge = Rect::from_min_size(inner.min, Vec2::splat(40.0));
    painter.rect_filled(badge, 12.0, theme::SAFFRON);
    icons::paint(painter, badge.shrink(10.0), Icon::Pen, theme::INK_BUTTON);
    widgets::kbd(
        painter,
        pos2(inner.right(), badge.center().y),
        "Ctrl N",
        false,
        true,
    );
    let body = painter.layout(
        t("quick-note-text").to_owned(),
        theme::font(14.0, Weight::Regular),
        Color32::from_rgb(0xC3, 0xC7, 0xD0),
        inner.width(),
    );
    let body_height = body.size().y;
    painter.galley(
        pos2(inner.left(), inner.bottom() - body_height),
        body,
        Color32::WHITE,
    );
    painter.text(
        pos2(inner.left(), inner.bottom() - body_height - 2.0),
        Align2::LEFT_BOTTOM,
        t("quick-note"),
        theme::display(20.0),
        Color32::WHITE,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("quick-note")));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        actions.push(Action::Command(Command::QuickNote));
    }
}

/// Zone en pointillés « Glissez un PDF ici » ; un clic ouvre le choix du fichier.
fn drop_zone(ui: &mut Ui, rect: Rect, data: &HomeData<'_>, actions: &mut Vec<Action>) {
    let dark = data.dark;
    let response = ui.interact(rect, ui.scope_id().with("depot-pdf"), Sense::click());
    let painter = ui.painter();
    let active = data.dragging_files;
    let (fill, border) = if active {
        (theme::accent_soft(dark), theme::accent(dark))
    } else if response.hovered() {
        (theme::hover(dark), theme::dashed(dark))
    } else {
        (theme::sidebar(dark), theme::dashed(dark))
    };
    painter.rect_filled(rect, theme::RADIUS_CARD, fill);
    widgets::dashed_rect(
        painter,
        rect.shrink(0.75),
        theme::RADIUS_CARD,
        Stroke::new(1.5, border),
    );
    let badge = Rect::from_min_size(
        pos2(rect.left() + 20.0, rect.center().y - 20.0),
        Vec2::splat(40.0),
    );
    painter.rect_filled(
        badge,
        12.0,
        if active {
            theme::surface(dark)
        } else {
            theme::chip(dark)
        },
    );
    icons::paint(
        painter,
        badge.shrink(10.0),
        Icon::Export,
        if active {
            theme::accent(dark)
        } else {
            theme::label_secondary(dark)
        },
    );
    let x = badge.right() + 16.0;
    let width = rect.right() - x - 16.0;
    let title = painter.layout(
        t("drop-pdf").to_owned(),
        theme::font(14.0, Weight::Semibold),
        theme::label(dark),
        width,
    );
    let text = painter.layout(
        t("drop-pdf-text").to_owned(),
        theme::font(14.0, Weight::Regular),
        theme::label_secondary(dark),
        width,
    );
    let total = title.size().y + text.size().y;
    let top = rect.center().y - total / 2.0;
    let title_height = title.size().y;
    painter.galley(pos2(x, top), title, theme::label(dark));
    painter.galley(
        pos2(x, top + title_height),
        text,
        theme::label_secondary(dark),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("drop-pdf")));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        actions.push(Action::Command(Command::ImportPdf));
    }
}

/// Grille de miniatures de pages (format 3:4), avec titre et cahier.
fn page_grid(
    ui: &mut Ui,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
    pages: &[NodeId],
    columns: usize,
    actions: &mut Vec<Action>,
) {
    let dark = data.dark;
    let gap = 18.0;
    let width = ui.available_width();
    let cell = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let thumb = vec2(cell, cell * 4.0 / 3.0);
    for chunk in pages.chunks(columns) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for id in chunk {
                let Some(node) = data.tree.get(*id) else {
                    continue;
                };
                let (rect, response) =
                    ui.allocate_exact_size(thumb + vec2(0.0, 52.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let page = Rect::from_min_size(rect.min, thumb);
                    ui.painter().add(
                        egui::epaint::Shadow {
                            offset: [0, 1],
                            blur: 3,
                            spread: 0,
                            color: Color32::from_black_alpha(14),
                        }
                        .as_shape(page, 12),
                    );
                    thumbs.paint(ui.ctx(), ui.painter(), page, *id, 12.0, dark);
                    let ring = if response.hovered() {
                        Color32::from_rgb(0xB9, 0xC0, 0xD0)
                    } else {
                        theme::border(dark)
                    };
                    ui.painter().rect_stroke(
                        page,
                        12.0,
                        Stroke::new(1.0, ring),
                        StrokeKind::Inside,
                    );
                    widgets::truncated(
                        ui.painter(),
                        pos2(page.left() + 2.0, page.bottom() + 10.0),
                        Align2::LEFT_TOP,
                        &node.name,
                        theme::font(14.0, Weight::Semibold),
                        theme::label(dark),
                        cell - 4.0,
                    );
                    let notebook = data.tree.notebook_of(*id).and_then(|n| data.tree.get(n));
                    if let Some(notebook) = notebook {
                        let y = page.bottom() + 38.0;
                        ui.painter().rect_filled(
                            Rect::from_center_size(pos2(page.left() + 6.0, y), Vec2::splat(8.0)),
                            2.0,
                            theme::to_color32(node_color(data.tree, notebook)),
                        );
                        widgets::truncated(
                            ui.painter(),
                            pos2(page.left() + 16.0, y),
                            Align2::LEFT_CENTER,
                            &notebook.name,
                            theme::font(12.0, Weight::Regular),
                            theme::label_secondary(dark),
                            cell - 18.0,
                        );
                    }
                }
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, &node.name)
                });
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    actions.push(Action::OpenPage(*id));
                }
            }
        });
        ui.add_space(18.0);
    }
}

/// Date de dernière modification d'un cahier : sa page la plus récemment écrite.
fn notebook_stamp(data: &HomeData<'_>, notebook: NodeId) -> Option<i64> {
    page_order(data.tree, notebook)
        .iter()
        .filter_map(|p| data.stamps.get(p).copied())
        .max()
}

fn notebooks_section(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    actions: &mut Vec<Action>,
) {
    let subjects = data.tree.children(None);
    ui.horizontal_wrapped(|ui| {
        section_title(ui, t("your-notebooks"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // De droite à gauche : les matières à l'envers, puis « Tous ».
            for subject in subjects.iter().rev() {
                let selected = state.filter == Some(subject.id);
                if widgets::chip(
                    ui,
                    &subject.name,
                    Some(theme::to_color32(subject.color)),
                    selected,
                    34.0,
                )
                .clicked()
                {
                    state.filter = if selected { None } else { Some(subject.id) };
                }
            }
            if widgets::chip(ui, t("all"), None, state.filter.is_none(), 34.0).clicked() {
                state.filter = None;
            }
        });
    });
    ui.add_space(18.0);
    let notebooks: Vec<&Node> = subjects
        .iter()
        .filter(|s| state.filter.map_or(true, |f| f == s.id))
        .flat_map(|s| data.tree.children(Some(s.id)))
        .collect();
    cover_grid(ui, state, data, &notebooks, true, actions);
}

fn cover_grid(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    notebooks: &[&Node],
    with_new: bool,
    actions: &mut Vec<Action>,
) {
    let gap = 22.0;
    let width = ui.available_width();
    let columns = ((width + gap) / (150.0 + gap)).floor().max(2.0) as usize;
    let cell = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let mut cells: Vec<Option<&Node>> = notebooks.iter().map(|n| Some(*n)).collect();
    if with_new {
        cells.push(None);
    }
    for chunk in cells.chunks(columns) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for cell_node in chunk {
                match cell_node {
                    Some(notebook) => notebook_cover(ui, state, data, notebook, cell, actions),
                    None => new_notebook_cell(ui, state, data, cell),
                }
            }
        });
        ui.add_space(28.0);
    }
}

fn notebook_cover(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    notebook: &Node,
    width: f32,
    actions: &mut Vec<Action>,
) {
    let dark = data.dark;
    let cover_size = vec2(width, width * 7.0 / 5.0);
    let (rect, response) = ui.allocate_exact_size(cover_size + vec2(0.0, 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let lift = if hovered { -4.0 } else { 0.0 };
        let angle = if hovered {
            (-0.6_f32).to_radians()
        } else {
            0.0
        };
        let cover = Rect::from_min_size(rect.min + vec2(0.0, lift), cover_size);
        let subject = data
            .tree
            .path(notebook.id)
            .into_iter()
            .find(|n| n.kind == NodeKind::Subject)
            .map_or("", |s| s.name.as_str());
        draw_cover(
            ui.painter(),
            cover,
            theme::to_color32(node_color(data.tree, notebook)),
            subject,
            &notebook.name,
            angle,
        );
        let pages = count_label(
            data.tree.page_count(notebook.id),
            t("page-one"),
            t("page-many"),
        );
        let meta = match notebook_stamp(data, notebook.id).and_then(dates::local) {
            Some(then) => format!("{pages} · {}", dates::relative(dates::now(), then)),
            None => pages,
        };
        widgets::truncated(
            ui.painter(),
            pos2(rect.left() + 2.0, rect.top() + cover_size.y + 10.0),
            Align2::LEFT_TOP,
            &meta,
            theme::font(13.0, Weight::Regular),
            theme::label_secondary(dark),
            width - 4.0,
        );
        if data.favorites.contains(&notebook.id) {
            icons::paint(
                ui.painter(),
                Rect::from_center_size(cover.right_top() + vec2(-16.0, 16.0), Vec2::splat(14.0)),
                Icon::Favorite,
                on_color(theme::to_color32(node_color(data.tree, notebook))).gamma_multiply(0.8),
            );
        }
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &notebook.name));
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        actions.push(Action::OpenNotebook(notebook.id));
    }
    response.context_menu(|ui| node_menu(ui, state, data, notebook, actions));
}

fn new_notebook_cell(ui: &mut Ui, state: &mut HomeState, data: &HomeData<'_>, width: f32) {
    let dark = data.dark;
    let cover_size = vec2(width, width * 7.0 / 5.0);
    let (rect, response) = ui.allocate_exact_size(cover_size + vec2(0.0, 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let cover = Rect::from_min_size(rect.min, cover_size);
        if response.hovered() {
            ui.painter()
                .rect_filled(cover, theme::cover_radius(1.0), theme::hover(dark));
        }
        widgets::dashed_rect(
            ui.painter(),
            cover.shrink(0.75),
            theme::cover_radius(1.0),
            Stroke::new(1.5, theme::dashed(dark)),
        );
        let circle = pos2(cover.center().x, cover.center().y - 14.0);
        ui.painter().circle(
            circle,
            22.0,
            theme::surface(dark),
            Stroke::new(1.0, theme::border(dark)),
        );
        icons::paint(
            ui.painter(),
            Rect::from_center_size(circle, Vec2::splat(20.0)),
            Icon::Plus,
            theme::accent(dark),
        );
        ui.painter().text(
            pos2(cover.center().x, circle.y + 38.0),
            Align2::CENTER_CENTER,
            t("new-notebook"),
            theme::font(14.0, Weight::Medium),
            theme::label_secondary(dark),
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("new-notebook")));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        state.open_new_notebook(data.tree, state.filter);
    }
}

// ---- Autres rubriques -------------------------------------------------------------------

fn view_title(ui: &mut Ui, text: &str, subtitle: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new(subtitle)
            .font(theme::font(14.0, Weight::Regular))
            .color(theme::label_secondary(dark)),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(text)
            .font(theme::display(40.0))
            .color(theme::label(dark)),
    );
    ui.add_space(32.0);
}

fn empty_message(ui: &mut Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new(text)
            .font(theme::font(15.0, Weight::Regular))
            .color(theme::label_secondary(dark)),
    );
}

fn recent_view(
    ui: &mut Ui,
    data: &HomeData<'_>,
    thumbs: &mut ThumbSource<'_>,
    actions: &mut Vec<Action>,
) {
    view_title(ui, t("recent"), t("recent-subtitle"));
    let pages: Vec<NodeId> = recent_pages(data, 36)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if pages.is_empty() {
        empty_message(ui, t("recent-empty"));
        return;
    }
    let columns = ((ui.available_width() + 18.0) / (176.0 + 18.0))
        .floor()
        .max(2.0) as usize;
    page_grid(ui, data, thumbs, &pages, columns, actions);
}

fn favorites_view(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    actions: &mut Vec<Action>,
) {
    view_title(ui, t("favorites"), t("favorites-subtitle"));
    let notebooks: Vec<&Node> = data
        .favorites
        .iter()
        .filter_map(|id| data.tree.get(*id))
        .collect();
    if notebooks.is_empty() {
        empty_message(ui, t("favorites-empty"));
        return;
    }
    cover_grid(ui, state, data, &notebooks, false, actions);
}

fn trash_view(ui: &mut Ui, data: &HomeData<'_>, actions: &mut Vec<Action>) {
    let dark = data.dark;
    ui.horizontal(|ui| {
        ui.vertical(|ui| view_title(ui, t("trash"), t("trash-subtitle")));
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if Button::new(t("empty-trash"), ButtonStyle::Secondary)
                .icon(Icon::Trash)
                .enabled(!data.trash.is_empty())
                .show(ui)
                .clicked()
            {
                actions.push(Action::EmptyTrash);
            }
        });
    });
    if data.trash.is_empty() {
        empty_message(ui, t("trash-empty"));
        return;
    }
    egui::Frame::new()
        .fill(theme::surface(dark))
        .stroke(Stroke::new(1.0, theme::border(dark)))
        .corner_radius(theme::RADIUS_CARD)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for node in data.trash {
                ui.horizontal(|ui| {
                    ui.set_min_height(48.0);
                    let icon = match node.kind {
                        NodeKind::Subject => Icon::Book,
                        NodeKind::Notebook => Icon::Notebook,
                        NodeKind::Folder => Icon::Folder,
                        NodeKind::Page => Icon::Page,
                    };
                    ui.add_space(8.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                    icons::paint(ui.painter(), rect, icon, theme::label_secondary(dark));
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&node.name).font(theme::font(15.0, Weight::Medium)),
                        )
                        .truncate(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, Icon::Close, t("delete-forever"), false, true)
                            .clicked()
                        {
                            actions.push(Action::DeleteForever(node.id));
                        }
                        if Button::new(t("restore"), ButtonStyle::Ghost)
                            .show(ui)
                            .clicked()
                        {
                            actions.push(Action::Restore(node.id));
                        }
                    });
                });
            }
        });
}

// ---- Menus ------------------------------------------------------------------------------

/// Menu d'une matière ou d'un cahier (clic droit).
fn node_menu(
    ui: &mut Ui,
    state: &mut HomeState,
    data: &HomeData<'_>,
    node: &Node,
    actions: &mut Vec<Action>,
) {
    let tree = data.tree;
    if ui.button(t("edit")).clicked() {
        let paper = data
            .papers
            .get(&node.id)
            .copied()
            .unwrap_or(Background::Blank);
        state.open_edit(tree, node, paper);
        ui.close();
    }
    if node.kind == NodeKind::Subject && ui.button(t("new-notebook-dots")).clicked() {
        state.open_new_notebook(tree, Some(node.id));
        ui.close();
    }
    if node.kind == NodeKind::Notebook {
        let favorite = data.favorites.contains(&node.id);
        let key = if favorite {
            "remove-favorite-notebook"
        } else {
            "add-favorite-notebook"
        };
        if ui.button(t(key)).clicked() {
            actions.push(Action::ToggleFavorite(node.id));
            ui.close();
        }
        let others: Vec<&Node> = tree
            .children(None)
            .into_iter()
            .filter(|s| Some(s.id) != node.parent_id)
            .collect();
        if !others.is_empty() {
            ui.menu_button(t("move-to"), |ui| {
                for subject in others {
                    if ui.button(subject.name.as_str()).clicked() {
                        actions.push(Action::Move(node.id, Some(subject.id), None));
                        ui.close();
                    }
                }
            });
        }
    }
    if ui.button(t("duplicate")).clicked() {
        actions.push(Action::Duplicate(node.id));
        ui.close();
    }
    if ui.button(t("move-up")).clicked() {
        actions.push(Action::MoveBy(node.id, -1));
        ui.close();
    }
    if ui.button(t("move-down")).clicked() {
        actions.push(Action::MoveBy(node.id, 1));
        ui.close();
    }
    ui.separator();
    if ui.button(t("import-pdf-here")).clicked() {
        actions.push(Action::ImportPdf(node.id));
        ui.close();
    }
    ui.menu_button(t("export"), |ui| {
        for (format, key) in [
            (ExportFormat::Pdf, "export-pdf"),
            (ExportFormat::Png, "export-png"),
            (ExportFormat::Svg, "export-svg"),
        ] {
            if ui.button(t(key)).clicked() {
                actions.push(Action::Export(node.id, format));
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button(t("to-trash")).clicked() {
        actions.push(Action::Trash(node.id));
        ui.close();
    }
}

// ---- Feuille de création ----------------------------------------------------------------

fn sheet_title(sheet: &Sheet) -> &'static str {
    match (sheet.target, sheet.kind) {
        (SheetTarget::Create, NodeKind::Subject) => t("sheet-new-subject"),
        (SheetTarget::Create, _) => t("sheet-new-notebook"),
        (SheetTarget::Edit(_), NodeKind::Subject) => t("sheet-edit-subject"),
        (SheetTarget::Edit(_), _) => t("sheet-edit-notebook"),
    }
}

fn glyph_label(icon: Icon) -> &'static str {
    match icon {
        Icon::Math => t("glyph-math"),
        Icon::Flask => t("glyph-flask"),
        Icon::Globe => t("glyph-globe"),
        Icon::Brush => t("glyph-brush"),
        Icon::Music => t("glyph-music"),
        Icon::Code => t("glyph-code"),
        Icon::Sport => t("glyph-sport"),
        Icon::Chat => t("glyph-chat"),
        Icon::Bulb => t("glyph-bulb"),
        Icon::Heart => t("glyph-heart"),
        Icon::Star => t("glyph-star"),
        _ => t("glyph-book"),
    }
}

fn field_label(ui: &mut Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new(text)
            .font(theme::font(13.0, Weight::Semibold))
            .color(theme::label(dark)),
    );
}

/// Feuille de création ou de modification (cahier ou matière), sur un voile sombre.
pub fn sheet_window(
    ctx: &egui::Context,
    state: &mut HomeState,
    tree: &LibraryTree,
    actions: &mut Vec<Action>,
) {
    let Some(mut sheet) = state.sheet.take() else {
        return;
    };
    let dark = ctx.global_style().visuals.dark_mode;
    let (mut done, mut cancel) = (false, false);
    let screen = ctx.content_rect();
    let width = (screen.width() - 32.0).min(860.0);
    let modal = egui::Modal::new(egui::Id::unique("calame-feuille"))
        .backdrop_color(theme::veil())
        .frame(
            egui::Frame::new()
                .fill(theme::surface(dark))
                .corner_radius(theme::RADIUS_SHEET)
                .shadow(theme::window_shadow()),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let preview_width = if width >= 760.0 { 300.0 } else { 0.0 };
            let form_width = width - preview_width;
            egui::ScrollArea::vertical()
                .max_height(screen.height() - 48.0)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        if preview_width > 0.0 {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(preview_width, 560.0), Sense::hover());
                            preview(ui, rect, &sheet, tree, dark);
                        }
                        ui.vertical(|ui| {
                            ui.set_width(form_width);
                            egui::Frame::new()
                                .inner_margin(egui::Margin {
                                    left: 32,
                                    right: 32,
                                    top: 30,
                                    bottom: 26,
                                })
                                .show(ui, |ui| {
                                    (done, cancel) = form(ui, &mut sheet, tree);
                                });
                        });
                    });
                });
        });
    if modal.should_close() {
        cancel = true;
    }
    if done {
        submit(sheet, actions);
    } else if !cancel {
        state.sheet = Some(sheet);
    }
}

fn submit(sheet: Sheet, actions: &mut Vec<Action>) {
    let name = sheet.name.trim().to_owned();
    match (sheet.target, sheet.kind) {
        (SheetTarget::Create, NodeKind::Subject) => actions.push(Action::CreateWith {
            parent: None,
            kind: NodeKind::Subject,
            name,
            color: sheet.color,
            icon: sheet.icon,
        }),
        (SheetTarget::Create, _) => {
            let subject = sheet.subject;
            if subject.is_none() {
                let subject_name = sheet.new_subject.trim();
                let subject_name = if subject_name.is_empty() {
                    t("welcome-subject")
                } else {
                    subject_name
                };
                actions.push(Action::CreateWith {
                    parent: None,
                    kind: NodeKind::Subject,
                    name: subject_name.to_owned(),
                    color: sheet.color,
                    icon: Icon::Book.key().to_owned(),
                });
            }
            actions.push(Action::CreateNotebook {
                subject,
                name,
                color: sheet.color,
                paper: sheet.paper,
            });
        }
        (SheetTarget::Edit(id), NodeKind::Notebook) => actions.push(Action::EditNotebook {
            id,
            name,
            color: sheet.color,
            subject: sheet.subject,
            paper: sheet.paper,
        }),
        (SheetTarget::Edit(id), _) => {
            actions.push(Action::Rename(id, name));
            actions.push(Action::SetStyle(id, sheet.color, sheet.icon));
        }
    }
}

/// Aperçu en direct : la couverture et, derrière, une page du papier choisi.
fn preview(ui: &mut Ui, rect: Rect, sheet: &Sheet, tree: &LibraryTree, dark: bool) {
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius {
            nw: theme::RADIUS_SHEET,
            sw: theme::RADIUS_SHEET,
            ne: 0,
            se: 0,
        },
        theme::app_background(dark),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, theme::border(dark)),
    );
    let color = theme::to_color32(sheet.color);
    let center = rect.center() - vec2(0.0, 14.0);
    if sheet.kind == NodeKind::Subject {
        let tile = Rect::from_center_size(center, Vec2::splat(112.0));
        icons::tile(painter, tile, color, &sheet.icon);
        let name = if sheet.name.trim().is_empty() {
            t("subject-name")
        } else {
            sheet.name.as_str()
        };
        widgets::truncated(
            painter,
            pos2(center.x, tile.bottom() + 22.0),
            Align2::CENTER_TOP,
            name,
            theme::display(20.0),
            theme::label(dark),
            rect.width() - 40.0,
        );
    } else {
        let group = Rect::from_center_size(center, vec2(218.0, 280.0));
        let page = Rect::from_min_size(group.min + vec2(22.0, 12.0), vec2(196.0, 268.0));
        painter.add(theme::card_shadow(dark).as_shape(page, 10));
        thumbs::paint_paper(painter, page, sheet.paper, 10.0, dark);
        let cover = Rect::from_min_size(group.min, vec2(196.0, 268.0));
        let subject = sheet
            .subject
            .and_then(|s| tree.get(s))
            .map_or(sheet.new_subject.as_str(), |s| s.name.as_str());
        let title = if sheet.name.trim().is_empty() {
            t("notebook-name")
        } else {
            sheet.name.as_str()
        };
        draw_cover(painter, cover, color, subject, title, 0.0);
    }
    painter.text(
        pos2(center.x, rect.bottom() - 36.0),
        Align2::CENTER_CENTER,
        t("preview"),
        theme::font(13.0, Weight::Regular),
        theme::label_secondary(dark),
    );
}

/// Formulaire de la feuille ; renvoie (valider, annuler).
fn form(ui: &mut Ui, sheet: &mut Sheet, tree: &LibraryTree) -> (bool, bool) {
    let dark = ui.visuals().dark_mode;
    let (mut done, mut cancel) = (false, false);
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(sheet_title(sheet))
                .font(theme::display(26.0))
                .color(theme::label(dark)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
            ui.painter().rect_filled(
                rect,
                10.0,
                if response.hovered() {
                    theme::hover(dark)
                } else {
                    theme::chip(dark)
                },
            );
            icons::paint(
                ui.painter(),
                rect.shrink(10.0),
                Icon::Close,
                theme::icon(dark),
            );
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, t("dismiss")));
            if response.on_hover_text(t("dismiss")).clicked() {
                cancel = true;
            }
        });
    });
    ui.add_space(14.0);

    field_label(
        ui,
        if sheet.kind == NodeKind::Subject {
            t("name")
        } else {
            t("title")
        },
    );
    let hint = if sheet.kind == NodeKind::Subject {
        t("subject-name")
    } else {
        t("notebook-name")
    };
    let name = widgets::text_field(ui, &mut sheet.name, hint, 46.0, 16.0);
    if sheet.fresh {
        name.request_focus();
        sheet.fresh = false;
    }
    let enter = name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.add_space(14.0);

    if sheet.kind == NodeKind::Notebook {
        field_label(ui, t("subject"));
        subject_picker(ui, sheet, tree);
        ui.add_space(14.0);
    }

    field_label(
        ui,
        if sheet.kind == NodeKind::Subject {
            t("color")
        } else {
            t("cover")
        },
    );
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (color, key) in theme::COVER_COLORS.iter().zip(theme::COVER_NAMES) {
            let selected = sheet.color == *color;
            if widgets::color_dot(ui, theme::to_color32(*color), 30.0, selected, t(key)).clicked() {
                sheet.color = *color;
            }
        }
    });
    ui.add_space(14.0);

    if sheet.kind == NodeKind::Subject {
        field_label(ui, t("icon"));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for glyph in icons::SUBJECT_GLYPHS {
                let selected = Icon::from_key(&sheet.icon) == Some(glyph);
                if widgets::icon_button(ui, glyph, glyph_label(glyph), selected, true).clicked() {
                    sheet.icon = glyph.key().to_owned();
                }
            }
        });
    } else {
        field_label(ui, t("paper"));
        paper_picker(ui, sheet);
    }

    ui.add_space(16.0);
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, theme::border(dark));
    ui.add_space(14.0);
    let valid = !sheet.name.trim().is_empty();
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let label = match sheet.target {
            SheetTarget::Create if sheet.kind == NodeKind::Notebook => t("create-and-write"),
            SheetTarget::Create => t("create"),
            SheetTarget::Edit(_) => t("save"),
        };
        let mut button = Button::new(label, ButtonStyle::Primary)
            .height(46.0)
            .enabled(valid);
        if sheet.target == SheetTarget::Create && sheet.kind == NodeKind::Notebook {
            button = button.trailing(Icon::ArrowRight);
        }
        if button.show(ui).clicked() || (enter && valid) {
            done = true;
        }
        if Button::new(t("cancel"), ButtonStyle::Ghost)
            .height(46.0)
            .show(ui)
            .clicked()
        {
            cancel = true;
        }
    });
    (done, cancel)
}

/// Liste déroulante des matières, avec « Nouvelle matière… ».
fn subject_picker(ui: &mut Ui, sheet: &mut Sheet, tree: &LibraryTree) {
    let dark = ui.visuals().dark_mode;
    let subjects = tree.children(None);
    if sheet.subject.is_none() {
        // Nouvelle matière : un champ pour son nom, et un lien pour revenir aux matières.
        widgets::text_field(ui, &mut sheet.new_subject, t("subject-name"), 46.0, 15.0);
        if !subjects.is_empty()
            && Button::new(t("choose-subject"), ButtonStyle::Link)
                .show(ui)
                .clicked()
        {
            sheet.subject = subjects.first().map(|s| s.id);
        }
        return;
    }
    let current = sheet.subject.and_then(|s| tree.get(s));
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    let fill = if response.hovered() {
        theme::hover(dark)
    } else {
        theme::surface(dark)
    };
    ui.painter().rect(
        rect,
        12.0,
        fill,
        Stroke::new(1.0, theme::field_border(dark)),
        StrokeKind::Inside,
    );
    if let Some(subject) = current {
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), Vec2::splat(10.0)),
            3.0,
            theme::to_color32(subject.color),
        );
        ui.painter().text(
            pos2(rect.left() + 34.0, rect.center().y),
            Align2::LEFT_CENTER,
            &subject.name,
            theme::font(15.0, Weight::Regular),
            theme::label(dark),
        );
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(
            pos2(rect.right() - 22.0, rect.center().y),
            Vec2::splat(16.0),
        ),
        Icon::ChevronDown,
        theme::label_secondary(dark),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::menu(&response).width(rect.width()).show(|ui| {
        for subject in &subjects {
            if ui
                .selectable_label(sheet.subject == Some(subject.id), subject.name.as_str())
                .clicked()
            {
                sheet.subject = Some(subject.id);
                if sheet.target == SheetTarget::Create {
                    sheet.color = subject.color;
                }
                ui.close();
            }
        }
        if sheet.target == SheetTarget::Create {
            ui.separator();
            if ui.button(t("new-subject-dots")).clicked() {
                sheet.subject = None;
                sheet.new_subject.clear();
                ui.close();
            }
        }
    });
}

/// Vignettes des papiers : uni, petits carreaux, lignes, pointillés, grands carreaux, isométrique.
fn paper_picker(ui: &mut Ui, sheet: &mut Sheet) {
    let dark = ui.visuals().dark_mode;
    let papers = [
        Background::Blank,
        Background::GridSmall,
        Background::Lines,
        Background::Dots,
        Background::GridLarge,
        Background::Isometric,
    ];
    debug_assert!(papers.iter().all(|p| PATTERN_BACKGROUNDS.contains(p)));
    let gap = 10.0;
    let width = ui.available_width();
    let columns = ((width + gap) / (76.0 + gap)).floor().clamp(3.0, 6.0) as usize;
    let cell = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    for chunk in papers.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for paper in chunk {
                let selected = sheet.paper == *paper;
                let (rect, response) =
                    ui.allocate_exact_size(vec2(cell, 76.0 + 24.0), Sense::click());
                let tile = Rect::from_min_size(rect.min, vec2(cell, 76.0));
                thumbs::paint_paper(ui.painter(), tile, *paper, 10.0, dark);
                let (ring, width) = if selected {
                    (theme::accent(dark), 2.0)
                } else if response.hovered() {
                    (theme::field_border(dark), 1.0)
                } else {
                    (theme::border(dark), 1.0)
                };
                ui.painter().rect_stroke(
                    tile,
                    10.0,
                    Stroke::new(width, ring),
                    if selected {
                        StrokeKind::Outside
                    } else {
                        StrokeKind::Inside
                    },
                );
                let label = t(background_key(*paper));
                ui.painter().text(
                    pos2(tile.center().x, tile.bottom() + 13.0),
                    Align2::CENTER_CENTER,
                    label,
                    theme::font(
                        12.0,
                        if selected {
                            Weight::Semibold
                        } else {
                            Weight::Regular
                        },
                    ),
                    if selected {
                        theme::accent(dark)
                    } else {
                        theme::label_secondary(dark)
                    },
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Button, true, selected, label)
                });
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    sheet.paper = *paper;
                }
            }
        });
        ui.add_space(6.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_pluralised() {
        assert_eq!(count_label(1, "cahier", "cahiers"), "1 cahier");
        assert_eq!(count_label(0, "cahier", "cahiers"), "0 cahiers");
        assert_eq!(count_label(3, "cahier", "cahiers"), "3 cahiers");
    }

    #[test]
    fn a_new_notebook_sheet_picks_the_first_subject_and_small_squares() {
        let mut state = HomeState::default();
        let tree = LibraryTree::default();
        state.open_new_notebook(&tree, None);
        let sheet = state.sheet.as_ref();
        assert!(sheet.is_some_and(|s| s.subject.is_none() && s.paper == Background::GridSmall));
    }
}
