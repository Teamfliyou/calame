//! Export et import de fichiers : opérations longues, exécutées hors du fil de l'interface.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;

use calame_core::{NodeId, Page};
use calame_export::{
    page_svg, render_pdf_pages, svg_to_png, svgs_to_pdf, PageBackground, RenderedPage, PNG_SCALE,
};

/// Pixels par unité de page pour les pages d'un PDF importé (1,5 : environ 144 points par pouce).
const IMPORT_PIXELS_PER_UNIT: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Png,
    Svg,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }
}

/// Une page prête à être exportée, avec son nom pour nommer les fichiers.
pub struct ExportPage {
    pub name: String,
    pub page: Page,
    pub background: PageBackground,
}

/// Où écrire : un fichier (PDF, ou page unique) ou un dossier (une image par page).
#[derive(Clone, Debug)]
pub enum Destination {
    File(PathBuf),
    Folder(PathBuf),
}

/// Nom proposé pour une sauvegarde : `calame-sauvegarde-AAAA-MM-JJ.zip`.
pub fn backup_file_name(unix_seconds: u64) -> String {
    let (year, month, day) = civil_from_days((unix_seconds / 86_400) as i64);
    format!("calame-sauvegarde-{year:04}-{month:02}-{day:02}.zip")
}

/// Date civile (année, mois, jour) à partir du nombre de jours écoulés depuis le 1er janvier 1970
/// (algorithme de Howard Hinnant, valable pour tout le calendrier grégorien).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Remplace les caractères interdits dans un nom de fichier.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "sans-titre".into()
    } else {
        cleaned.into()
    }
}

/// Écrit les pages au format demandé et renvoie un message de réussite.
pub fn export_pages(
    pages: &[ExportPage],
    format: ExportFormat,
    destination: &Destination,
) -> Result<String, String> {
    if pages.is_empty() {
        return Err("Aucune page à exporter.".into());
    }
    let svgs: Vec<String> = pages
        .iter()
        .map(|p| page_svg(&p.page, &p.background))
        .collect();
    let count = pages.len();
    let plural = if count > 1 { "s" } else { "" };
    match (format, destination) {
        (ExportFormat::Pdf, Destination::File(path)) => {
            let pdf = svgs_to_pdf(&svgs).map_err(|e| e.to_string())?;
            write(path, &pdf)?;
            Ok(format!(
                "{count} page{plural} exportée{plural} : {}",
                path.display()
            ))
        }
        (ExportFormat::Pdf, Destination::Folder(_)) => {
            Err("Un PDF s'écrit dans un fichier, pas dans un dossier.".into())
        }
        (_, Destination::File(path)) if count == 1 => {
            let bytes = encode_page(&svgs[0], format)?;
            write(path, &bytes)?;
            Ok(format!("Page exportée : {}", path.display()))
        }
        (_, Destination::File(_)) => Err("Plusieurs pages s'exportent dans un dossier.".into()),
        (_, Destination::Folder(dir)) => {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let width = count.to_string().len().max(2);
            for (i, (svg, page)) in svgs.iter().zip(pages).enumerate() {
                let file = format!(
                    "{:0width$}-{}.{}",
                    i + 1,
                    sanitize_file_name(&page.name),
                    format.extension()
                );
                write(&dir.join(file), &encode_page(svg, format)?)?;
            }
            Ok(format!(
                "{count} page{plural} exportée{plural} dans {}",
                dir.display()
            ))
        }
    }
}

fn encode_page(svg: &str, format: ExportFormat) -> Result<Vec<u8>, String> {
    match format {
        ExportFormat::Png => svg_to_png(svg, PNG_SCALE).map_err(|e| e.to_string()),
        ExportFormat::Svg => Ok(svg.as_bytes().to_vec()),
        ExportFormat::Pdf => svgs_to_pdf(&[svg.to_owned()]).map_err(|e| e.to_string()),
    }
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("Écriture de {} impossible : {e}", path.display()))
}

/// Lit un PDF et rend toutes ses pages en images ; renvoie le nom du document (nom du fichier).
pub fn render_pdf_file(path: &Path) -> Result<(String, Vec<RenderedPage>), String> {
    let bytes =
        fs::read(path).map_err(|e| format!("Lecture de {} impossible : {e}", path.display()))?;
    let pages = render_pdf_pages(bytes, IMPORT_PIXELS_PER_UNIT).map_err(|e| e.to_string())?;
    let name = path.file_stem().map_or_else(
        || "PDF importé".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    Ok((name, pages))
}

/// Résultat d'une tâche de fond, à appliquer par l'interface.
pub enum TaskResult {
    Message(String),
    ImportedPdf {
        parent: NodeId,
        name: String,
        pages: Vec<RenderedPage>,
    },
    Failed(String),
}

/// Tâche en cours dans un fil séparé, avec de quoi l'afficher et en récupérer le résultat.
pub struct Task {
    pub label: String,
    receiver: Receiver<TaskResult>,
}

impl Task {
    pub fn spawn(label: String, work: impl FnOnce() -> TaskResult + Send + 'static) -> Self {
        let (sender, receiver) = channel();
        thread::spawn(move || {
            // Si la fenêtre s'est fermée entre-temps, personne n'écoute plus : ce n'est pas une erreur.
            let _ = sender.send(work());
        });
        Self { label, receiver }
    }

    /// Le résultat s'il est prêt. Une tâche dont le fil a planté est un échec.
    pub fn poll(&self) -> Option<TaskResult> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(TaskResult::Failed("La tâche s'est interrompue.".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::time::{Duration, Instant};

    use calame_core::{Background, Brush, Item, Point2, Stroke, StrokePoint};

    use super::*;

    fn page_named(name: &str) -> ExportPage {
        let mut stroke = Stroke::new(Brush::default());
        for (x, y) in [(50.0, 50.0), (200.0, 120.0)] {
            stroke.push(StrokePoint {
                pos: Point2::new(x, y),
                pressure: 0.5,
                tilt: None,
            });
        }
        ExportPage {
            name: name.into(),
            page: Page {
                items: vec![Item::Stroke(stroke)],
            },
            background: PageBackground::Pattern(Background::GridSmall),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("calame-files-{tag}-{}", NodeId::new()));
        fs::create_dir_all(&dir).expect("dossier");
        dir
    }

    #[test]
    fn backup_names_carry_the_date() {
        assert_eq!(backup_file_name(0), "calame-sauvegarde-1970-01-01.zip");
        assert_eq!(
            backup_file_name(1_000_000_000),
            "calame-sauvegarde-2001-09-09.zip"
        );
        assert_eq!(
            backup_file_name(19_723 * 86_400),
            "calame-sauvegarde-2024-01-01.zip"
        );
        // 29 février d'une année bissextile, et jour suivant.
        assert_eq!(
            backup_file_name(19_782 * 86_400 + 5),
            "calame-sauvegarde-2024-02-29.zip"
        );
        assert_eq!(
            backup_file_name(19_783 * 86_400),
            "calame-sauvegarde-2024-03-01.zip"
        );
        assert_eq!(backup_file_name(86_399), "calame-sauvegarde-1970-01-01.zip");
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(
            sanitize_file_name("Maths: chap. 1 / 2?"),
            "Maths_ chap. 1 _ 2_"
        );
        assert_eq!(sanitize_file_name("  .. "), "sans-titre");
        assert_eq!(sanitize_file_name(""), "sans-titre");
        assert_eq!(sanitize_file_name("Géométrie"), "Géométrie");
    }

    #[test]
    fn multi_page_pdf_goes_into_one_file() {
        let dir = temp_dir("pdf");
        let path = dir.join("cours.pdf");
        let pages = [page_named("A"), page_named("B")];
        let message = export_pages(&pages, ExportFormat::Pdf, &Destination::File(path.clone()))
            .expect("export");
        assert!(message.starts_with("2 pages exportées"));
        let bytes = fs::read(&path).expect("fichier");
        assert!(bytes.starts_with(b"%PDF-"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn png_and_svg_exports_name_one_file_per_page() {
        let dir = temp_dir("png");
        let pages = [page_named("Intro"), page_named("Exercice: 1")];
        export_pages(&pages, ExportFormat::Png, &Destination::Folder(dir.clone())).expect("PNG");
        export_pages(&pages, ExportFormat::Svg, &Destination::Folder(dir.clone())).expect("SVG");
        let mut names: Vec<String> = fs::read_dir(&dir)
            .expect("lecture")
            .map(|e| {
                e.expect("entrée")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "01-Intro.png",
                "01-Intro.svg",
                "02-Exercice_ 1.png",
                "02-Exercice_ 1.svg"
            ]
        );
        let png = fs::read(dir.join("01-Intro.png")).expect("fichier");
        assert_eq!(&png[1..4], b"PNG");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_single_page_can_go_to_a_single_file() {
        let dir = temp_dir("single");
        let path = dir.join("page.svg");
        export_pages(
            &[page_named("P")],
            ExportFormat::Svg,
            &Destination::File(path.clone()),
        )
        .expect("export");
        assert!(fs::read_to_string(&path)
            .expect("fichier")
            .starts_with("<svg"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn incompatible_destinations_and_empty_exports_are_errors() {
        let dir = temp_dir("err");
        let two = [page_named("A"), page_named("B")];
        assert!(export_pages(
            &two,
            ExportFormat::Png,
            &Destination::File(dir.join("x.png"))
        )
        .is_err());
        assert!(export_pages(&two, ExportFormat::Pdf, &Destination::Folder(dir.clone())).is_err());
        assert!(export_pages(
            &[],
            ExportFormat::Pdf,
            &Destination::File(dir.join("x.pdf"))
        )
        .is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pdf_file_is_rendered_page_by_page() {
        let dir = temp_dir("import");
        let path = dir.join("Mon cours.pdf");
        let pages = [page_named("A"), page_named("B"), page_named("C")];
        export_pages(&pages, ExportFormat::Pdf, &Destination::File(path.clone())).expect("PDF");
        let (name, rendered) = render_pdf_file(&path).expect("import");
        assert_eq!(name, "Mon cours");
        assert_eq!(rendered.len(), 3);
        assert!(render_pdf_file(&dir.join("absent.pdf")).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tasks_report_their_result_or_their_failure() {
        let ok = Task::spawn("test".into(), || TaskResult::Message("fini".into()));
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(r) = ok.poll() {
                break r;
            }
            assert!(Instant::now() < deadline, "tâche trop lente");
            thread::sleep(Duration::from_millis(5));
        };
        assert!(matches!(result, TaskResult::Message(m) if m == "fini"));

        let crashed = Task::spawn("plantage".into(), || panic!("échec volontaire du test"));
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(r) = crashed.poll() {
                break r;
            }
            assert!(Instant::now() < deadline, "tâche trop lente");
            thread::sleep(Duration::from_millis(5));
        };
        assert!(matches!(result, TaskResult::Failed(_)));
    }
}
