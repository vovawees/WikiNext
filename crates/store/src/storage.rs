use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageStatus {
    pub root: PathBuf,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("не удалось создать каталог {path}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("не удалось проверить каталог {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("путь {0} является символической ссылкой")]
    Symlink(PathBuf),
    #[error("путь {0} не является каталогом")]
    NotDirectory(PathBuf),
    #[error("небезопасный путь локального хранилища {0}")]
    UnsafePath(PathBuf),
    #[error(
        "каталог {path} имеет небезопасные права {actual:#o}; \
         ожидается отдельный приватный каталог с правами 0o700"
    )]
    UnsafePermissions { path: PathBuf, actual: u32 },
    #[error("каталог {path} недоступен для записи")]
    NotWritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Debug)]
pub struct LocalStorage {
    root: Arc<PathBuf>,
    temporary: Arc<PathBuf>,
}

impl LocalStorage {
    pub async fn prepare(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let requested_root = root.into();
        let root = absolute_storage_path(&requested_root)?;
        inspect_existing_ancestors(&root).await?;
        ensure_directory(&root).await?;
        let canonical_root =
            fs::canonicalize(&root)
                .await
                .map_err(|source| StorageError::Inspect {
                    path: root.clone(),
                    source,
                })?;
        if canonical_root != root {
            return Err(StorageError::Symlink(requested_root));
        }

        let blobs = canonical_root.join("blobs");
        ensure_directory(&blobs).await?;

        let temporary = canonical_root.join("tmp");
        ensure_directory(&temporary).await?;

        let storage = Self {
            root: Arc::new(canonical_root),
            temporary: Arc::new(temporary),
        };
        storage.diagnose().await?;
        Ok(storage)
    }

    pub async fn status(&self) -> Result<StorageStatus, StorageError> {
        inspect_private_directory(&self.root).await?;
        inspect_private_directory(&self.root.join("blobs")).await?;
        inspect_private_directory(&self.temporary).await?;

        Ok(StorageStatus {
            root: self.root.as_ref().clone(),
        })
    }

    pub async fn diagnose(&self) -> Result<StorageStatus, StorageError> {
        let status = self.status().await?;
        probe_writable(&self.temporary).await?;
        Ok(status)
    }
}

fn absolute_storage_path(path: &Path) -> Result<PathBuf, StorageError> {
    let components: Vec<_> = path.components().collect();
    if path.as_os_str().is_empty()
        || components
            .iter()
            .any(|component| matches!(component, Component::ParentDir))
        || !components
            .iter()
            .any(|component| matches!(component, Component::Normal(_)))
    {
        return Err(StorageError::UnsafePath(path.to_owned()));
    }

    let mut absolute = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().map_err(|source| StorageError::Inspect {
            path: path.to_owned(),
            source,
        })?
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => return Err(StorageError::UnsafePath(path.to_owned())),
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                absolute.push(component.as_os_str());
            }
        }
    }

    if absolute.parent().is_none() {
        return Err(StorageError::UnsafePath(path.to_owned()));
    }
    Ok(absolute)
}

async fn inspect_existing_ancestors(path: &Path) -> Result<(), StorageError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current).await {
            Ok(metadata) => validate_metadata(&current, &metadata)?,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(StorageError::Inspect {
                    path: current,
                    source,
                });
            }
        }
    }
    Ok(())
}

async fn ensure_directory(path: &Path) -> Result<(), StorageError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) => {
            validate_metadata(path, &metadata)?;
            validate_directory_permissions(path, &metadata)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder
                .create(path)
                .await
                .map_err(|source| StorageError::Create {
                    path: path.to_owned(),
                    source,
                })?;
            inspect_private_directory(path).await
        }
        Err(source) => Err(StorageError::Inspect {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(unix)]
fn validate_directory_permissions(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;

    let actual = metadata.permissions().mode() & 0o7777;
    if actual != 0o700 {
        return Err(StorageError::UnsafePermissions {
            path: path.to_owned(),
            actual,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_directory_permissions(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), StorageError> {
    Ok(())
}

async fn inspect_private_directory(path: &Path) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|source| StorageError::Inspect {
            path: path.to_owned(),
            source,
        })?;
    validate_metadata(path, &metadata)?;
    validate_directory_permissions(path, &metadata)
}

fn validate_metadata(path: &Path, metadata: &std::fs::Metadata) -> Result<(), StorageError> {
    if metadata.file_type().is_symlink() {
        return Err(StorageError::Symlink(path.to_owned()));
    }
    if !metadata.is_dir() {
        return Err(StorageError::NotDirectory(path.to_owned()));
    }
    Ok(())
}

async fn probe_writable(directory: &Path) -> Result<(), StorageError> {
    let probe = directory.join(format!(".wikinext-probe-{}", Uuid::new_v4()));
    let file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
        .await
        .map_err(|source| StorageError::NotWritable {
            path: directory.to_owned(),
            source,
        })?;

    if let Err(source) = file.sync_all().await {
        drop(file);
        let _ = fs::remove_file(&probe).await;
        return Err(StorageError::NotWritable {
            path: directory.to_owned(),
            source,
        });
    }
    drop(file);

    fs::remove_file(&probe)
        .await
        .map_err(|source| StorageError::NotWritable {
            path: directory.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!("wikinext-storage-test-{}", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn prepares_content_addressed_storage_directories() {
        let root = temporary_path();
        let storage = LocalStorage::prepare(&root)
            .await
            .expect("storage preparation succeeds");

        assert!(root.join("blobs").is_dir());
        assert!(root.join("tmp").is_dir());
        storage.diagnose().await.expect("storage is writable");

        fs::remove_dir_all(root)
            .await
            .expect("temporary directory cleanup succeeds");
    }

    #[tokio::test]
    async fn rejects_regular_file_as_storage_root() {
        let root = temporary_path();
        fs::write(&root, b"not a directory")
            .await
            .expect("fixture creation succeeds");

        assert!(matches!(
            LocalStorage::prepare(&root).await,
            Err(StorageError::NotDirectory(_))
        ));

        fs::remove_file(root)
            .await
            .expect("temporary file cleanup succeeds");
    }

    #[tokio::test]
    async fn rejects_parent_directory_traversal_before_writing() {
        assert!(matches!(
            LocalStorage::prepare("/tmp/..").await,
            Err(StorageError::UnsafePath(_))
        ));
        assert!(matches!(
            LocalStorage::prepare(".").await,
            Err(StorageError::UnsafePath(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symbolic_link_in_ancestor() {
        use std::os::unix::fs::symlink;

        let fixture = temporary_path();
        let target = fixture.join("target");
        let link = fixture.join("link");
        fs::create_dir_all(&target)
            .await
            .expect("target directory creation succeeds");
        symlink(&target, &link).expect("symlink creation succeeds");

        assert!(matches!(
            LocalStorage::prepare(link.join("data")).await,
            Err(StorageError::Symlink(_))
        ));

        fs::remove_dir_all(fixture)
            .await
            .expect("temporary directory cleanup succeeds");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn creates_private_storage_directories() {
        use std::os::unix::fs::PermissionsExt;

        let root = temporary_path();
        let storage = LocalStorage::prepare(&root)
            .await
            .expect("storage preparation succeeds");

        for path in [&root, &root.join("blobs"), &root.join("tmp")] {
            let mode = fs::metadata(path)
                .await
                .expect("directory metadata is available")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
        }

        storage.status().await.expect("storage status is healthy");
        fs::remove_dir_all(root)
            .await
            .expect("temporary directory cleanup succeeds");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_existing_shared_directory_without_changing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = temporary_path();
        fs::create_dir(&root)
            .await
            .expect("fixture directory creation succeeds");
        fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
            .await
            .expect("fixture permissions are set");

        assert!(matches!(
            LocalStorage::prepare(&root).await,
            Err(StorageError::UnsafePermissions { actual: 0o755, .. })
        ));
        let actual = fs::metadata(&root)
            .await
            .expect("fixture metadata is available")
            .permissions()
            .mode()
            & 0o7777;
        assert_eq!(actual, 0o755);
        assert!(!root.join("blobs").exists());
        assert!(!root.join("tmp").exists());

        fs::remove_dir(root)
            .await
            .expect("temporary directory cleanup succeeds");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn detects_permissions_changed_after_startup() {
        use std::os::unix::fs::PermissionsExt;

        let root = temporary_path();
        let storage = LocalStorage::prepare(&root)
            .await
            .expect("storage preparation succeeds");
        fs::set_permissions(root.join("tmp"), std::fs::Permissions::from_mode(0o755))
            .await
            .expect("fixture permissions are changed");

        assert!(matches!(
            storage.status().await,
            Err(StorageError::UnsafePermissions { actual: 0o755, .. })
        ));

        fs::remove_dir_all(root)
            .await
            .expect("temporary directory cleanup succeeds");
    }
}
