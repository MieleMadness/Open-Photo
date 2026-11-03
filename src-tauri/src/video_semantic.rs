//! Bounded, resumable frame embeddings. Staged frames are invisible until a whole video commits.
use crate::{ai_embeddings, ai_inference, ai_preprocess, video_engine, video_probe};
use rusqlite::{params, Connection};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::Duration,
};
pub(crate) fn key() -> String {
    format!(
        "video-samples-v1:{}:{}:{}",
        crate::image_decode::POLICY,
        crate::ai_models::SUPPORTED_CLIP_VISION_SHA256,
        video_engine::VERSION
    )
}
pub(crate) fn is_video(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi"
    )
}
pub(crate) fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS video_frames(root TEXT,path TEXT,modified INTEGER,bytes INTEGER,model TEXT,seconds REAL,vector BLOB,PRIMARY KEY(root,path,seconds));
 CREATE TABLE IF NOT EXISTS video_index_state(root TEXT,path TEXT,modified INTEGER,bytes INTEGER,model TEXT,PRIMARY KEY(root,path));
 CREATE TABLE IF NOT EXISTS video_frame_work(root TEXT,path TEXT,modified INTEGER,bytes INTEGER,model TEXT,idx INTEGER,seconds REAL,hash INTEGER,r INTEGER,g INTEGER,b INTEGER,vector BLOB,PRIMARY KEY(root,path,idx));
 CREATE INDEX IF NOT EXISTS idx_video_frame_model ON video_frames(model);").map_err(|e|e.to_string())
}
pub(crate) fn samples(duration: f64) -> Result<Vec<f64>, String> {
    if !duration.is_finite() || duration <= 0. || duration > 7200. {
        return Err("Video content indexing currently supports videos up to two hours".into());
    }
    let count = (duration.ceil() as usize).clamp(3, 48);
    Ok((0..count)
        .map(|i| duration * (i as f64 + 0.5) / count as f64)
        .collect())
}
fn signature(image: &image::DynamicImage) -> (u64, [u8; 3]) {
    let small = image
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut hash = 0u64;
    let mut means = [0u32; 3];
    for y in 0..8 {
        for x in 0..8 {
            let p = small.get_pixel(x, y);
            let q = small.get_pixel(x + 1, y);
            hash = (hash << 1)
                | u64::from(
                    p.0.iter().map(|v| u32::from(*v)).sum::<u32>()
                        > q.0.iter().map(|v| u32::from(*v)).sum(),
                );
            for c in 0..3 {
                means[c] += u32::from(p[c]);
            }
        }
    }
    (hash, means.map(|v| (v / 64) as u8))
}
fn similar(a: (u64, [u8; 3]), b: (u64, [u8; 3])) -> bool {
    (a.0 ^ b.0).count_ones() <= 2 && (0..3).all(|c| a.1[c].abs_diff(b.1[c]) <= 3)
}
pub(crate) fn frame(
    engine: &Path,
    source: &Path,
    stream: usize,
    seconds: f64,
    cancel: impl Fn() -> bool,
) -> Result<(image::DynamicImage, f64), String> {
    if let Some(image) = frame_once(engine, source, stream, seconds, &cancel)? {
        return Ok((image, seconds));
    }
    // Sparse/VFR tails can have no subsequent decoded frame. Use a real first frame
    // and its own timestamp rather than inventing a match at the requested time.
    frame_once(engine, source, stream, 0., cancel)?
        .map(|image| (image, 0.))
        .ok_or("No decodable video frame".into())
}
fn frame_once(
    engine: &Path,
    source: &Path,
    stream: usize,
    seconds: f64,
    cancel: impl Fn() -> bool,
) -> Result<Option<image::DynamicImage>, String> {
    let args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-threads",
        "2",
        "-filter_threads",
        "1",
        "-max_alloc",
        "268435456",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov,matroska,webm,avi",
        "-ss",
    ]
    .into_iter()
    .map(Into::into)
    .chain([
        format!("{seconds:.6}").into(),
        "-i".into(),
        source.into(),
        "-map".into(),
        format!("0:{stream}").into(),
        "-an".into(),
        "-sn".into(),
        "-dn".into(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        "scale=640:640:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1"
            .into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "-c:v".into(),
        "png".into(),
        "-threads:v".into(),
        "1".into(),
        "-f".into(),
        "image2pipe".into(),
        "pipe:1".into(),
    ])
    .collect();
    let refs = args
        .iter()
        .map(OsString::as_os_str)
        .collect::<Vec<&OsStr>>();
    let bytes = video_engine::run_until(
        &video_engine::tool(engine, "ffmpeg")?,
        &refs,
        Duration::from_secs(30),
        cancel,
    )?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > 640 || h > 640 {
        return Err("Video frame exceeds its decode bounds".into());
    }
    image::load_from_memory(&bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}
struct Plan {
    root: String,
    path: String,
    modified: i64,
    bytes: u64,
    info: video_probe::VideoInfo,
    positions: Vec<f64>,
}
pub(crate) struct IndexWorker {
    db: Connection,
    session: ort::session::Session,
    engine: PathBuf,
    plan: Option<Plan>,
}
impl IndexWorker {
    pub(crate) fn new(device: &str, engine: PathBuf) -> Result<Self, String> {
        video_engine::tool(&engine, "ffmpeg")?;
        Ok(Self {
            db: ai_embeddings::database()?,
            session: ai_inference::semantic_image_session(Some(device))?,
            engine,
            plan: None,
        })
    }
    /// One frame is a scheduling unit; returning false yields the inference slot before the next.
    pub(crate) fn process(
        &mut self,
        root: &str,
        path: &str,
        modified: i64,
        bytes: u64,
        cancel: impl Fn() -> bool,
    ) -> Result<bool, String> {
        crate::ai_index::verify_source(root, path, modified, bytes)?;
        let model = key();
        let current:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM video_index_state WHERE root=?1 AND path=?2 AND modified=?3 AND bytes=?4 AND model=?5)",params![root,path,modified,bytes,model],|r|r.get(0)).map_err(|e|e.to_string())?;
        if current {
            return Ok(true);
        }
        if !self.plan.as_ref().is_some_and(|p| {
            p.root == root && p.path == path && p.modified == modified && p.bytes == bytes
        }) {
            let source = crate::media_stream::validated_video(root, path)?;
            let info = video_probe::probe(&self.engine, &source, &cancel)?;
            if info.hdr {
                return Err("HDR video content indexing needs tested tone mapping".into());
            }
            let positions = samples(info.duration_seconds)?;
            self.plan = Some(Plan {
                root: root.into(),
                path: path.into(),
                modified,
                bytes,
                info,
                positions,
            });
        }
        self.db.execute("DELETE FROM video_frame_work WHERE root=?1 AND path=?2 AND (modified!=?3 OR bytes!=?4 OR model!=?5)",params![root,path,modified,bytes,model]).map_err(|e|e.to_string())?;
        let next = self
            .db
            .query_row(
                "SELECT MAX(idx) FROM video_frame_work WHERE root=?1 AND path=?2",
                params![root, path],
                |r| r.get::<_, Option<usize>>(0),
            )
            .map_err(|e| e.to_string())?
            .map_or(0, |i| i + 1);
        let plan = self.plan.as_ref().unwrap();
        if next >= plan.positions.len() {
            return self.publish(root, path, modified, bytes, &model);
        }
        let seconds = plan.positions[next];
        let source = crate::media_stream::validated_video(root, path)?;
        let (image, seconds) = frame(
            &self.engine,
            &source,
            plan.info.video_stream,
            seconds,
            &cancel,
        )?;
        let sig = signature(&image);
        let duplicate = {
            let mut stmt=self.db.prepare("SELECT hash,r,g,b FROM video_frame_work WHERE root=?1 AND path=?2 AND vector IS NOT NULL").map_err(|e|e.to_string())?;
            let rows = stmt
                .query_map(params![root, path], |r| {
                    Ok((
                        r.get::<_, i64>(0)? as u64,
                        [r.get(1)?, r.get(2)?, r.get(3)?],
                    ))
                })
                .map_err(|e| e.to_string())?;
            let mut found = false;
            for row in rows {
                if similar(sig, row.map_err(|e| e.to_string())?) {
                    found = true;
                    break;
                }
            }
            found
        };
        let vector = if duplicate {
            None
        } else {
            let values = ai_preprocess::clip_tensor(image)?;
            let result = ai_inference::run_semantic_tensor(&mut self.session, values)?;
            Some(
                result
                    .values
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
        };
        if cancel() {
            return Err("Video indexing cancelled".into());
        }
        crate::ai_index::verify_source(root, path, modified, bytes)?;
        self.db.execute("INSERT OR REPLACE INTO video_frame_work VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![root,path,modified,bytes,model,next,seconds,sig.0 as i64,sig.1[0],sig.1[1],sig.1[2],vector]).map_err(|e|e.to_string())?;
        if next + 1 == plan.positions.len() {
            self.publish(root, path, modified, bytes, &model)
        } else {
            Ok(false)
        }
    }
    fn publish(
        &mut self,
        root: &str,
        path: &str,
        modified: i64,
        bytes: u64,
        model: &str,
    ) -> Result<bool, String> {
        crate::ai_index::verify_source(root, path, modified, bytes)?;
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM video_frames WHERE root=?1 AND path=?2",
            params![root, path],
        )
        .map_err(|e| e.to_string())?;
        let count=tx.execute("INSERT INTO video_frames SELECT root,path,modified,bytes,model,seconds,vector FROM video_frame_work WHERE root=?1 AND path=?2 AND modified=?3 AND bytes=?4 AND model=?5 AND vector IS NOT NULL",params![root,path,modified,bytes,model]).map_err(|e|e.to_string())?;
        if count == 0 {
            return Err("No representative video frames were indexed".into());
        }
        tx.execute(
            "INSERT OR REPLACE INTO video_index_state VALUES(?1,?2,?3,?4,?5)",
            params![root, path, modified, bytes, model],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM video_frame_work WHERE root=?1 AND path=?2",
            params![root, path],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let folder =
            std::env::temp_dir().join(format!("openphoto-semantic-{}", crate::video_project::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("scenes 世界.mp4");
        let args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=red:s=96x64:r=30:d=0.2",
            "-f",
            "lavfi",
            "-i",
            "color=blue:s=96x64:r=30:d=0.2",
            "-f",
            "lavfi",
            "-i",
            "color=yellow:s=96x64:r=30:d=0.2",
            "-filter_complex",
            "[0:v][1:v][2:v]concat=n=3:v=1:a=0[v]",
            "-map",
            "[v]",
            "-c:v",
            "libopenh264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ]
        .into_iter()
        .map(Into::into)
        .chain([source.clone().into()])
        .collect();
        video_engine::run(
            &video_engine::tool(&engine, "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(20),
        )
        .unwrap();
        (engine, folder, source)
    }
    #[test]
    #[ignore = "prepared local model CPU acceptance; run explicitly"]
    fn video_embeddings_resume_staging_and_publish_with_the_real_bundled_model() {
        let bundle = std::env::var_os("OPENPHOTO_AI_BUNDLE_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ai-models")
            });
        crate::ai_models::configure_bundled_directory(bundle);
        let (engine, folder, source) = fixture();
        let root = folder.to_str().unwrap();
        let path = source.to_str().unwrap();
        let meta = source.metadata().unwrap();
        let modified = meta
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let bytes = meta.len();
        let before = crate::video_project::fingerprint(&source, || false).unwrap();
        let database = folder.join("frames.sqlite");
        let db = Connection::open(&database).unwrap();
        initialize(&db).unwrap();
        let mut worker = IndexWorker {
            db,
            session: ai_inference::semantic_image_session(Some("cpu")).unwrap(),
            engine: engine.clone(),
            plan: None,
        };
        assert!(!worker
            .process(root, path, modified, bytes, || false)
            .unwrap());
        assert!(worker
            .process(root, path, modified, bytes, || true)
            .is_err());
        assert_eq!(
            worker
                .db
                .query_row("SELECT COUNT(*) FROM video_frames", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(worker);
        let db = Connection::open(&database).unwrap();
        initialize(&db).unwrap();
        let mut worker = IndexWorker {
            db,
            session: ai_inference::semantic_image_session(Some("cpu")).unwrap(),
            engine,
            plan: None,
        };
        let mut complete = false;
        for _ in 0..3 {
            if worker
                .process(root, path, modified, bytes, || false)
                .unwrap()
            {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(
            worker
                .db
                .query_row(
                    "SELECT COUNT(*) FROM video_frames WHERE length(vector)=2048",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            3
        );
        assert_eq!(
            worker
                .db
                .query_row("SELECT COUNT(*) FROM video_frame_work", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(worker);
        assert_eq!(
            crate::video_project::fingerprint(&source, || false).unwrap(),
            before
        );
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(database).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn short_real_frames_reach_each_scene_preserve_sources_and_cancel_cleanly() {
        let (engine, folder, source) = fixture();
        let before = crate::video_project::fingerprint(&source, || false).unwrap();
        let mut signatures = vec![];
        for position in samples(0.6).unwrap() {
            let (image, time) = frame(&engine, &source, 0, position, || false).unwrap();
            assert!((time - position).abs() < 0.001);
            assert!(image.width() <= 640 && image.height() <= 640);
            signatures.push(signature(&image));
        }
        assert!(!similar(signatures[0], signatures[1]));
        assert!(!similar(signatures[1], signatures[2]));
        let (_, time) = frame(&engine, &source, 0, 2., || false).unwrap();
        assert_eq!(time, 0.);
        assert!(frame(&engine, &source, 0, 0.1, || true).is_err());
        assert_eq!(
            crate::video_project::fingerprint(&source, || false).unwrap(),
            before
        );
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn sampling_is_bounded_and_reaches_the_middle_and_end() {
        let short = samples(0.6).unwrap();
        assert_eq!(short.len(), 3);
        assert!((short[1] - 0.3).abs() < 0.0001);
        let long = samples(7200.).unwrap();
        assert_eq!(long.len(), 48);
        assert!(long[47] > 7000.);
        assert!(samples(f64::NAN).is_err());
    }
    #[test]
    fn dedup_keeps_different_colors_even_when_their_edge_hashes_match() {
        let a = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            20,
            20,
            image::Rgb([255, 0, 0]),
        ));
        let b = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            20,
            20,
            image::Rgb([0, 0, 255]),
        ));
        assert!(similar(signature(&a), signature(&a)));
        assert!(!similar(signature(&a), signature(&b)));
    }
    #[test]
    fn staging_is_not_a_searchable_partial_video() {
        let db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        db.execute(
            "INSERT INTO video_frame_work VALUES('r','p',10,5,'model',0,0.2,0,1,2,3,?1)",
            [vec![0u8; 2048]],
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM video_frames", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
