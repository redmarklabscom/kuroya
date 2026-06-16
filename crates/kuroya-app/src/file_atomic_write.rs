use anyhow::{Context, bail};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncWriteExt;

use kuroya_core::TextSnapshot;

const SAVE_TEMP_CREATE_ATTEMPTS: u32 = 32;
static SAVE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) async fn write_text_snapshot_atomic_async(
    path: &Path,
    text: TextSnapshot,
) -> anyhow::Result<()> {
    validate_save_target_async(path).await?;

    let (temp, mut file) = create_unique_temp_file_async(path).await?;
    let result = async {
        for chunk in text.chunks() {
            file.write_all(chunk.as_bytes()).await?;
        }
        file.sync_all().await?;
        drop(file);
        let target_metadata = ensure_target_allows_atomic_replace_async(path).await?;
        preserve_permissions_async(target_metadata.as_ref(), &temp).await?;
        tokio::fs::rename(&temp, path).await?;
        sync_parent_dir_best_effort_async(path).await;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }

    result
}

pub(crate) fn write_text_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    validate_save_target(path)?;

    let (temp, mut file) = create_unique_temp_file(path)?;
    let result = (|| -> anyhow::Result<()> {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        let target_metadata = ensure_target_allows_atomic_replace(path)?;
        preserve_permissions(target_metadata.as_ref(), &temp)?;
        std::fs::rename(&temp, path)?;
        sync_parent_dir_best_effort(path);
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }

    result
}

fn create_unique_temp_file(path: &Path) -> anyhow::Result<(PathBuf, File)> {
    create_unique_temp_file_from_candidates(
        path,
        (0..SAVE_TEMP_CREATE_ATTEMPTS).map(|attempt| temporary_path(path, attempt)),
    )
}

fn create_unique_temp_file_from_candidates<I>(
    path: &Path,
    candidates: I,
) -> anyhow::Result<(PathBuf, File)>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut collisions = 0;
    for temp in candidates {
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collisions += 1;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary save file {}", temp.display())
                });
            }
        }
    }

    bail!(
        "failed to create unique temporary save file near {} after {} collisions",
        path.display(),
        collisions
    )
}

async fn create_unique_temp_file_async(path: &Path) -> anyhow::Result<(PathBuf, tokio::fs::File)> {
    let mut collisions = 0;
    for attempt in 0..SAVE_TEMP_CREATE_ATTEMPTS {
        let temp = temporary_path(path, attempt);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await
        {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collisions += 1;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary save file {}", temp.display())
                });
            }
        }
    }

    bail!(
        "failed to create unique temporary save file near {} after {} collisions",
        path.display(),
        collisions
    )
}

fn validate_save_target(path: &Path) -> anyhow::Result<()> {
    ensure_path_names_file(path)?;
    ensure_parent_dir(path)?;
    ensure_target_allows_atomic_replace(path).map(|_| ())
}

async fn validate_save_target_async(path: &Path) -> anyhow::Result<()> {
    ensure_path_names_file(path)?;
    ensure_parent_dir_async(path).await?;
    ensure_target_allows_atomic_replace_async(path)
        .await
        .map(|_| ())
}

fn ensure_path_names_file(path: &Path) -> anyhow::Result<()> {
    if path.file_name().is_none() {
        bail!("save path must name a file: {}", path.display());
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    let Some(parent) = parent_dir(path) else {
        return Ok(());
    };
    match std::fs::metadata(parent) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => bail!("save parent path is not a directory: {}", parent.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create save parent directory {}",
                    parent.display()
                )
            })?;
            match std::fs::metadata(parent).with_context(|| {
                format!(
                    "failed to inspect save parent directory {}",
                    parent.display()
                )
            })? {
                metadata if metadata.is_dir() => Ok(()),
                _ => bail!("save parent path is not a directory: {}", parent.display()),
            }
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect save parent directory {}",
                parent.display()
            )
        }),
    }
}

async fn ensure_parent_dir_async(path: &Path) -> anyhow::Result<()> {
    let Some(parent) = parent_dir(path) else {
        return Ok(());
    };
    match tokio::fs::metadata(parent).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => bail!("save parent path is not a directory: {}", parent.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create save parent directory {}",
                    parent.display()
                )
            })?;
            match tokio::fs::metadata(parent).await.with_context(|| {
                format!(
                    "failed to inspect save parent directory {}",
                    parent.display()
                )
            })? {
                metadata if metadata.is_dir() => Ok(()),
                _ => bail!("save parent path is not a directory: {}", parent.display()),
            }
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect save parent directory {}",
                parent.display()
            )
        }),
    }
}

fn ensure_target_allows_atomic_replace(path: &Path) -> anyhow::Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            bail!("save target is a directory: {}", path.display())
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("save target is a symlink: {}", path.display())
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!("save target is not a file: {}", path.display())
        }
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect save target {}", path.display()))
        }
    }
}

async fn ensure_target_allows_atomic_replace_async(
    path: &Path,
) -> anyhow::Result<Option<std::fs::Metadata>> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.is_dir() => {
            bail!("save target is a directory: {}", path.display())
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("save target is a symlink: {}", path.display())
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!("save target is not a file: {}", path.display())
        }
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect save target {}", path.display()))
        }
    }
}

fn preserve_permissions(
    target_metadata: Option<&std::fs::Metadata>,
    temp: &Path,
) -> anyhow::Result<()> {
    if let Some(metadata) = target_metadata {
        std::fs::set_permissions(temp, metadata.permissions())?;
    }
    Ok(())
}

async fn preserve_permissions_async(
    target_metadata: Option<&std::fs::Metadata>,
    temp: &Path,
) -> anyhow::Result<()> {
    if let Some(metadata) = target_metadata {
        tokio::fs::set_permissions(temp, metadata.permissions()).await?;
    }
    Ok(())
}

fn sync_parent_dir_best_effort(path: &Path) {
    if let Some(parent) = parent_dir(path) {
        let _ = sync_dir(parent);
    }
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> anyhow::Result<()> {
    let dir = File::open(path)?;
    dir.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

async fn sync_parent_dir_best_effort_async(path: &Path) {
    if let Some(parent) = parent_dir(path) {
        let _ = sync_dir_async(parent).await;
    }
}

#[cfg(unix)]
async fn sync_dir_async(path: &Path) -> anyhow::Result<()> {
    let dir = tokio::fs::File::open(path).await?;
    dir.sync_all().await?;
    Ok(())
}

#[cfg(not(unix))]
async fn sync_dir_async(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn parent_dir(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn temporary_path(path: &Path, attempt: u32) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = SAVE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    temporary_path_with_unique(path, unique, sequence, attempt)
}

fn temporary_path_with_unique(path: &Path, unique: u128, sequence: u64, attempt: u32) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kuroya-save");
    path.with_file_name(format!(
        ".{file_name}.tmp.{}.{}.{}.{}",
        std::process::id(),
        unique,
        sequence,
        attempt
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        create_unique_temp_file_from_candidates, ensure_target_allows_atomic_replace,
        ensure_target_allows_atomic_replace_async, temporary_path_with_unique,
        validate_save_target,
    };
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "kuroya-atomic-write-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .join("src")
            .join("main.rs")
    }

    fn remove_root(path: &Path) {
        let root = path.parent().and_then(Path::parent).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn atomic_temp_creation_skips_existing_candidate_without_truncating_it() {
        let path = temp_file("collision");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let stale = temporary_path_with_unique(&path, 17, 23, 0);
        let next = temporary_path_with_unique(&path, 17, 23, 1);
        fs::write(&stale, b"stale crash temp").unwrap();

        let (temp, mut file) =
            create_unique_temp_file_from_candidates(&path, [stale.clone(), next.clone()]).unwrap();
        file.write_all(b"new save temp").unwrap();
        drop(file);

        assert_eq!(temp, next);
        assert_eq!(fs::read(&stale).unwrap(), b"stale crash temp");
        assert_eq!(fs::read(&next).unwrap(), b"new save temp");

        remove_root(&path);
    }

    #[test]
    fn save_target_validation_allows_missing_and_existing_file_targets() {
        let path = temp_file("regular-target");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        validate_save_target(&path).unwrap();

        fs::write(&path, b"existing").unwrap();
        validate_save_target(&path).unwrap();

        remove_root(&path);
    }

    #[test]
    fn target_replace_validation_returns_metadata_for_existing_targets_only() {
        let path = temp_file("target-metadata");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        assert!(
            ensure_target_allows_atomic_replace(&path)
                .unwrap()
                .is_none()
        );

        fs::write(&path, b"existing").unwrap();
        let metadata = ensure_target_allows_atomic_replace(&path)
            .unwrap()
            .expect("existing file metadata");

        assert!(metadata.is_file());

        remove_root(&path);
    }

    #[tokio::test]
    async fn async_target_replace_validation_returns_metadata_for_existing_targets_only() {
        let path = temp_file("async-target-metadata");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        assert!(
            ensure_target_allows_atomic_replace_async(&path)
                .await
                .unwrap()
                .is_none()
        );

        fs::write(&path, b"existing").unwrap();
        let metadata = ensure_target_allows_atomic_replace_async(&path)
            .await
            .unwrap()
            .expect("existing file metadata");

        assert!(metadata.is_file());

        remove_root(&path);
    }

    #[test]
    fn save_target_validation_rejects_non_file_targets() {
        let path = temp_file("directory-target");
        fs::create_dir_all(&path).unwrap();

        let error = validate_save_target(&path).unwrap_err().to_string();

        assert!(error.contains("save target is a directory"));
        assert!(error.contains(&path.display().to_string()));

        remove_root(&path);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_rejects_file_symlink_target_without_replacing_link() {
        use std::os::unix::fs::symlink;

        let path = temp_file("symlink-target");
        let linked = path.with_file_name("linked.rs");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&linked, b"linked").unwrap();
        symlink(&linked, &path).unwrap();

        let error = super::write_text_atomic(&path, "replacement")
            .unwrap_err()
            .to_string();

        assert!(error.contains("save target is a symlink"));
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&linked).unwrap(), b"linked");

        remove_root(&path);
    }
}
