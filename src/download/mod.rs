//! Bounded, resumable downloads with integrity verification and atomic publication.
//!
//! The manager is intentionally independent of Minecraft metadata. Callers turn
//! their metadata into a [`DownloadPlan`] and consume the progress channel.

use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, Url, header};
use serde::{Deserialize, Serialize};
use sha1::Digest as _;
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Semaphore, mpsc},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

const PART_SUFFIX: &str = "andromeda.part";
const RESUME_SUFFIX: &str = "andromeda.resume.json";

/// An integrity value supplied by trusted metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    Sha1(String),
    Sha256(String),
}

impl Checksum {
    fn validate(&self) -> Result<(), DownloadError> {
        let (name, value, length) = match self {
            Self::Sha1(value) => ("SHA-1", value, 40),
            Self::Sha256(value) => ("SHA-256", value, 64),
        };
        if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DownloadError::InvalidPlan(format!(
                "invalid {name} checksum"
            )));
        }
        Ok(())
    }
}

/// A single immutable transfer description.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub id: String,
    pub url: Url,
    /// Path below the manager's configured destination root.
    pub destination: PathBuf,
    pub expected_size: Option<u64>,
    pub checksum: Option<Checksum>,
}

/// A validated set of independent downloads.
#[derive(Debug, Clone, Default)]
pub struct DownloadPlan {
    pub jobs: Vec<DownloadJob>,
}

/// Runtime policy. Limits are validated by [`DownloadManager::new`].
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub concurrency: usize,
    pub max_attempts: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub attempt_timeout: Duration,
    pub initial_backoff: Duration,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            concurrency: 8,
            max_attempts: 4,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(5 * 60),
            initial_backoff: Duration::from_millis(250),
        }
    }
}

/// A bounded-channel event. Slow consumers may miss intermediate byte events;
/// terminal events are still attempted and the final summary is authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEvent {
    Started { id: String, resumed_from: u64 },
    Progress(DownloadProgress),
    Retrying {
        id: String,
        attempt: usize,
        delay: Duration,
        reason: String,
    },
    Completed { id: String, bytes: u64 },
    Failed { id: String, error: String },
    Cancelled { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub completed_jobs: usize,
    pub total_jobs: usize,
    /// Bytes represented by completed/resumed output.
    pub logical_bytes: u64,
    /// Bytes transferred during this execution only.
    pub network_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadSummary {
    pub completed_jobs: usize,
    pub logical_bytes: u64,
    pub network_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("invalid download plan: {0}")]
    InvalidPlan(String),
    #[error("download was cancelled")]
    Cancelled,
    #[error("request failed: {0}")]
    Network(String),
    #[error("server returned HTTP {status}")]
    Http {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    #[error("download timed out")]
    Timeout,
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("integrity check failed for {path}: {reason}")]
    Integrity { path: PathBuf, reason: String },
    #[error("one or more downloads failed: {0}")]
    Batch(String),
    #[error("download worker failed: {0}")]
    Worker(String),
}

impl DownloadError {
    fn retryable(&self) -> bool {
        match self {
            Self::Network(_) | Self::Timeout => true,
            Self::Http { status, .. } => {
                *status == StatusCode::REQUEST_TIMEOUT
                    || *status == StatusCode::TOO_MANY_REQUESTS
                    || status.is_server_error()
            }
            Self::Io { source, .. } => matches!(
                source.kind(),
                std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::UnexpectedEof
            ),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadManager {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    root: PathBuf,
    config: DownloadConfig,
    client: Client,
}

#[derive(Debug, Default)]
struct Counters {
    logical: AtomicU64,
    network: AtomicU64,
    completed: AtomicUsize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResumeRecord {
    url: String,
    validator: String,
}

#[derive(Debug)]
struct AttemptOutput {
    bytes: u64,
}

impl DownloadManager {
    pub fn new(root: impl Into<PathBuf>, config: DownloadConfig) -> Result<Self, DownloadError> {
        if !(1..=64).contains(&config.concurrency) {
            return Err(DownloadError::InvalidPlan(
                "concurrency must be between 1 and 64".into(),
            ));
        }
        if !(1..=16).contains(&config.max_attempts) {
            return Err(DownloadError::InvalidPlan(
                "max_attempts must be between 1 and 16".into(),
            ));
        }
        let client = Client::builder()
            .user_agent(concat!("Andromeda-Client/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(config.connect_timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|error| DownloadError::Network(error.to_string()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                root: root.into(),
                config,
                client,
            }),
        })
    }

    /// Executes all jobs with bounded concurrency. Dropping the progress receiver
    /// never stops downloads; use the cancellation token for cooperative stop.
    pub async fn execute(
        &self,
        plan: DownloadPlan,
        cancellation: CancellationToken,
        progress: mpsc::Sender<DownloadEvent>,
    ) -> Result<DownloadSummary, DownloadError> {
        validate_plan(&plan)?;
        fs::create_dir_all(&self.inner.root)
            .await
            .map_err(|source| io_error(&self.inner.root, source))?;
        reject_symlink(&self.inner.root).await?;

        let total_jobs = plan.jobs.len();
        let total_bytes = plan
            .jobs
            .iter()
            .try_fold(0_u64, |sum, job| job.expected_size.and_then(|n| sum.checked_add(n)));
        let counters = Arc::new(Counters::default());
        let semaphore = Arc::new(Semaphore::new(self.inner.config.concurrency));
        let mut tasks = JoinSet::new();

        for job in plan.jobs {
            let inner = Arc::clone(&self.inner);
            let counters = Arc::clone(&counters);
            let semaphore = Arc::clone(&semaphore);
            let progress = progress.clone();
            let cancellation = cancellation.clone();
            tasks.spawn(async move {
                let permit = tokio::select! {
                    () = cancellation.cancelled() => return Err(DownloadError::Cancelled),
                    permit = semaphore.acquire_owned() => permit.map_err(|error| DownloadError::Worker(error.to_string()))?,
                };
                let result = run_job(
                    &inner,
                    &job,
                    &cancellation,
                    &progress,
                    &counters,
                    total_jobs,
                    total_bytes,
                )
                .await;
                drop(permit);
                match &result {
                    Ok(output) => {
                        counters.completed.fetch_add(1, Ordering::Relaxed);
                        send_event(&progress, DownloadEvent::Completed {
                            id: job.id.clone(),
                            bytes: output.bytes,
                        });
                        send_progress(&progress, &counters, total_jobs, total_bytes);
                    }
                    Err(DownloadError::Cancelled) => {
                        send_event(&progress, DownloadEvent::Cancelled { id: job.id.clone() });
                    }
                    Err(error) => send_event(&progress, DownloadEvent::Failed {
                        id: job.id.clone(),
                        error: error.to_string(),
                    }),
                }
                result
            });
        }
        drop(progress);

        let mut failures = Vec::new();
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(DownloadError::Cancelled)) => failures.push("cancelled".to_owned()),
                Ok(Err(error)) => failures.push(error.to_string()),
                Err(error) => failures.push(format!("worker: {error}")),
            }
        }
        if cancellation.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        if !failures.is_empty() {
            return Err(DownloadError::Batch(failures.join("; ")));
        }
        Ok(DownloadSummary {
            completed_jobs: counters.completed.load(Ordering::Relaxed),
            logical_bytes: counters.logical.load(Ordering::Relaxed),
            network_bytes: counters.network.load(Ordering::Relaxed),
        })
    }
}

async fn run_job(
    inner: &Inner,
    job: &DownloadJob,
    cancellation: &CancellationToken,
    progress: &mpsc::Sender<DownloadEvent>,
    counters: &Counters,
    total_jobs: usize,
    total_bytes: Option<u64>,
) -> Result<AttemptOutput, DownloadError> {
    let destination = inner.root.join(&job.destination);
    ensure_safe_parent(&inner.root, &destination).await?;
    let part = side_path(&destination, PART_SUFFIX)?;
    let resume = side_path(&destination, RESUME_SUFFIX)?;
    reject_unsafe_existing_file(&destination).await?;
    reject_unsafe_existing_file(&part).await?;
    reject_unsafe_existing_file(&resume).await?;

    if fs::try_exists(&destination)
        .await
        .map_err(|source| io_error(&destination, source))?
        && verify(&destination, job).await.is_ok()
    {
        let bytes = file_len(&destination).await?;
        counters.logical.fetch_add(bytes, Ordering::Relaxed);
        return Ok(AttemptOutput { bytes });
    }

    let mut last_error = None;
    let mut logical_accounted = 0_u64;
    for attempt in 1..=inner.config.max_attempts {
        if cancellation.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let result = tokio::time::timeout(
            inner.config.attempt_timeout,
            transfer_once(
                inner,
                job,
                &part,
                &resume,
                cancellation,
                progress,
                counters,
                total_jobs,
                total_bytes,
                &mut logical_accounted,
            ),
        )
        .await
        .unwrap_or(Err(DownloadError::Timeout));
        match result {
            Ok(()) => {
                verify(&part, job).await?;
                atomic_commit(&part, &destination).await?;
                let _ = fs::remove_file(&resume).await;
                return Ok(AttemptOutput {
                    bytes: file_len(&destination).await?,
                });
            }
            Err(error) if error.retryable() && attempt < inner.config.max_attempts => {
                let delay = retry_after(&error)
                    .unwrap_or_else(|| backoff(inner.config.initial_backoff, attempt));
                send_event(progress, DownloadEvent::Retrying {
                    id: job.id.clone(),
                    attempt: attempt + 1,
                    delay,
                    reason: error.to_string(),
                });
                last_error = Some(error);
                tokio::select! {
                    () = cancellation.cancelled() => return Err(DownloadError::Cancelled),
                    () = tokio::time::sleep(delay) => {}
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| DownloadError::Worker("attempt loop ended".into())))
}

#[allow(clippy::too_many_arguments)]
async fn transfer_once(
    inner: &Inner,
    job: &DownloadJob,
    part: &Path,
    resume_path: &Path,
    cancellation: &CancellationToken,
    progress: &mpsc::Sender<DownloadEvent>,
    counters: &Counters,
    total_jobs: usize,
    total_bytes: Option<u64>,
    logical_accounted: &mut u64,
) -> Result<(), DownloadError> {
    let existing = file_len_or_zero(part).await?;
    let record = read_resume(resume_path).await;
    let can_resume = existing > 0
        && record
            .as_ref()
            .is_some_and(|record| record.url == job.url.as_str() && !record.validator.is_empty());
    let offset = if can_resume { existing } else { 0 };

    let mut request = inner.client.get(job.url.clone());
    if offset > 0 {
        let validator = &record.as_ref().expect("checked above").validator;
        request = request
            .header(header::RANGE, format!("bytes={offset}-"))
            .header(header::IF_RANGE, validator);
    }
    let response = tokio::select! {
        () = cancellation.cancelled() => return Err(DownloadError::Cancelled),
        response = request.send() => response.map_err(|error| DownloadError::Network(error.to_string()))?,
    };
    if !allowed_url(response.url()) {
        return Err(DownloadError::InvalidPlan(format!(
            "redirected to a disallowed URL: {}",
            response.url()
        )));
    }
    let status = response.status();
    if !status.is_success() {
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        return Err(DownloadError::Http {
            status,
            retry_after,
        });
    }
    let resumed = offset > 0
        && status == StatusCode::PARTIAL_CONTENT
        && content_range_starts_at(response.headers(), offset);
    let write_offset = if resumed { offset } else { 0 };
    let validator = response
        .headers()
        .get(header::ETAG)
        .or_else(|| response.headers().get(header::LAST_MODIFIED))
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if resumed {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(part)
        .await
        .map_err(|source| io_error(part, source))?;
    if let Some(validator) = validator {
        let record = ResumeRecord {
            url: job.url.to_string(),
            validator,
        };
        write_resume(resume_path, &record).await?;
    } else {
        let _ = fs::remove_file(resume_path).await;
    }
    send_event(progress, DownloadEvent::Started {
        id: job.id.clone(),
        resumed_from: write_offset,
    });
    match write_offset.cmp(logical_accounted) {
        std::cmp::Ordering::Greater => {
            counters
                .logical
                .fetch_add(write_offset - *logical_accounted, Ordering::Relaxed);
        }
        std::cmp::Ordering::Less => {
            counters
                .logical
                .fetch_sub(*logical_accounted - write_offset, Ordering::Relaxed);
        }
        std::cmp::Ordering::Equal => {}
    }
    *logical_accounted = write_offset;
    if write_offset > 0 {
        send_progress(progress, counters, total_jobs, total_bytes);
    }

    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::select! {
            () = cancellation.cancelled() => return Err(DownloadError::Cancelled),
            result = tokio::time::timeout(inner.config.idle_timeout, stream.next()) => result.map_err(|_| DownloadError::Timeout)?,
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|error| DownloadError::Network(error.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|source| io_error(part, source))?;
        let count = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        counters.network.fetch_add(count, Ordering::Relaxed);
        counters.logical.fetch_add(count, Ordering::Relaxed);
        *logical_accounted = logical_accounted.saturating_add(count);
        send_progress(progress, counters, total_jobs, total_bytes);
    }
    file.flush().await.map_err(|source| io_error(part, source))?;
    file.sync_all().await.map_err(|source| io_error(part, source))?;
    Ok(())
}

fn validate_plan(plan: &DownloadPlan) -> Result<(), DownloadError> {
    let mut destinations = HashSet::new();
    let mut ids = HashSet::new();
    for job in &plan.jobs {
        if job.id.is_empty() || !ids.insert(&job.id) {
            return Err(DownloadError::InvalidPlan("job IDs must be non-empty and unique".into()));
        }
        validate_relative_path(&job.destination)?;
        let folded = job.destination.to_string_lossy().to_ascii_lowercase();
        if !destinations.insert(folded) {
            return Err(DownloadError::InvalidPlan(format!(
                "duplicate destination: {}",
                job.destination.display()
            )));
        }
        if !allowed_url(&job.url) {
            return Err(DownloadError::InvalidPlan(format!(
                "URL must use HTTPS: {}",
                job.url
            )));
        }
        if let Some(checksum) = &job.checksum {
            checksum.validate()?;
        }
    }
    Ok(())
}

fn allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        || (url.scheme() == "http"
            && url.host_str().is_some_and(|host| {
                host == "localhost"
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            }))
}

/// Rejects traversal, absolute paths, platform prefixes and Windows device names
/// on every platform so plans behave consistently when moved between hosts.
pub fn validate_relative_path(path: &Path) -> Result<(), DownloadError> {
    if path.as_os_str().is_empty() || path.as_os_str().to_string_lossy().len() > 1024 {
        return Err(DownloadError::InvalidPlan("destination path is empty or too long".into()));
    }
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(DownloadError::InvalidPlan(format!("unsafe destination: {}", path.display())));
        };
        let value = value.to_string_lossy();
        if value.is_empty()
            || value.len() > 255
            || value.ends_with(['.', ' '])
            || value.contains(['\0', ':'])
            || is_windows_device_name(&value)
        {
            return Err(DownloadError::InvalidPlan(format!("unsafe path component: {value}")));
        }
    }
    Ok(())
}

fn is_windows_device_name(value: &str) -> bool {
    let stem = value.split('.').next().unwrap_or_default().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.strip_prefix("COM").is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        || stem.strip_prefix("LPT").is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

async fn ensure_safe_parent(root: &Path, destination: &Path) -> Result<(), DownloadError> {
    let relative_parent = destination
        .strip_prefix(root)
        .map_err(|_| DownloadError::InvalidPlan("destination escaped root".into()))?
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let mut current = root.to_path_buf();
    for component in relative_parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current).await {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(DownloadError::InvalidPlan(format!(
                    "destination parent is not a real directory: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .await
                    .map_err(|source| io_error(&current, source))?;
            }
            Err(source) => return Err(io_error(&current, source)),
        }
    }
    Ok(())
}

async fn reject_unsafe_existing_file(path: &Path) -> Result<(), DownloadError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(DownloadError::InvalidPlan(format!(
                "download file is not a regular file: {}",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path, source)),
    }
}

async fn reject_symlink(path: &Path) -> Result<(), DownloadError> {
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DownloadError::InvalidPlan(
            "download root must be a real directory".into(),
        ));
    }
    Ok(())
}

async fn verify(path: &Path, job: &DownloadJob) -> Result<(), DownloadError> {
    let actual_size = file_len(path).await?;
    if let Some(expected) = job.expected_size
        && actual_size != expected
    {
        return Err(DownloadError::Integrity {
            path: path.to_path_buf(),
            reason: format!("expected {expected} bytes, received {actual_size}"),
        });
    }
    if let Some(checksum) = &job.checksum {
        let (expected, actual) = match checksum {
            Checksum::Sha1(expected) => (expected, digest_file::<sha1::Sha1>(path).await?),
            Checksum::Sha256(expected) => (expected, digest_file::<sha2::Sha256>(path).await?),
        };
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(DownloadError::Integrity {
                path: path.to_path_buf(),
                reason: format!("checksum mismatch (expected {expected}, got {actual})"),
            });
        }
    }
    Ok(())
}

async fn digest_file<D>(path: &Path) -> Result<String, DownloadError>
where
    D: sha2::Digest + Default,
{
    let mut file = fs::File::open(path)
        .await
        .map_err(|source| io_error(path, source))?;
    let mut digest = D::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|source| io_error(path, source))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

async fn atomic_commit(part: &Path, destination: &Path) -> Result<(), DownloadError> {
    // rename is atomic on the same volume; staging beside the destination enforces that.
    #[cfg(windows)]
    if fs::try_exists(destination)
        .await
        .map_err(|source| io_error(destination, source))?
    {
        fs::remove_file(destination)
            .await
            .map_err(|source| io_error(destination, source))?;
    }
    fs::rename(part, destination)
        .await
        .map_err(|source| io_error(destination, source))
}

async fn read_resume(path: &Path) -> Option<ResumeRecord> {
    let bytes = fs::read(path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

async fn write_resume(path: &Path, record: &ResumeRecord) -> Result<(), DownloadError> {
    let bytes = serde_json::to_vec(record)
        .map_err(|error| DownloadError::Worker(error.to_string()))?;
    fs::write(path, bytes)
        .await
        .map_err(|source| io_error(path, source))
}

fn content_range_starts_at(headers: &header::HeaderMap, offset: u64) -> bool {
    headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse::<u64>().ok())
        == Some(offset)
}

fn side_path(destination: &Path, suffix: &str) -> Result<PathBuf, DownloadError> {
    let name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| DownloadError::InvalidPlan("destination filename is not Unicode".into()))?;
    Ok(destination.with_file_name(format!(".{name}.{suffix}")))
}

async fn file_len(path: &Path) -> Result<u64, DownloadError> {
    fs::metadata(path)
        .await
        .map(|metadata| metadata.len())
        .map_err(|source| io_error(path, source))
}

async fn file_len_or_zero(path: &Path) -> Result<u64, DownloadError> {
    match fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(io_error(path, source)),
    }
}

fn retry_after(error: &DownloadError) -> Option<Duration> {
    match error {
        DownloadError::Http { retry_after, .. } => *retry_after,
        _ => None,
    }
}

fn backoff(initial: Duration, attempt: usize) -> Duration {
    let exponent = u32::try_from(attempt.saturating_sub(1).min(10)).unwrap_or(10);
    initial.saturating_mul(2_u32.pow(exponent))
}

fn send_event(sender: &mpsc::Sender<DownloadEvent>, event: DownloadEvent) {
    let _ = sender.try_send(event);
}

fn send_progress(
    sender: &mpsc::Sender<DownloadEvent>,
    counters: &Counters,
    total_jobs: usize,
    total_bytes: Option<u64>,
) {
    send_event(
        sender,
        DownloadEvent::Progress(DownloadProgress {
            completed_jobs: counters.completed.load(Ordering::Relaxed),
            total_jobs,
            logical_bytes: counters.logical.load(Ordering::Relaxed),
            network_bytes: counters.network.load(Ordering::Relaxed),
            total_bytes,
        }),
    );
}

fn io_error(path: &Path, source: std::io::Error) -> DownloadError {
    DownloadError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths() {
        for path in ["../escape", "/absolute", "safe/../escape", "CON", "a/NUL.txt", "trailing. "] {
            assert!(validate_relative_path(Path::new(path)).is_err(), "{path}");
        }
        assert!(validate_relative_path(Path::new("libraries/org/example.jar")).is_ok());
    }

    #[test]
    fn validates_checksums() {
        assert!(Checksum::Sha1("a".repeat(40)).validate().is_ok());
        assert!(Checksum::Sha256("F".repeat(64)).validate().is_ok());
        assert!(Checksum::Sha1("x".repeat(40)).validate().is_err());
    }

    #[test]
    fn classifies_retries() {
        assert!(
            DownloadError::Http {
                status: StatusCode::TOO_MANY_REQUESTS,
                retry_after: None
            }
            .retryable()
        );
        assert!(
            !DownloadError::Http {
                status: StatusCode::BAD_REQUEST,
                retry_after: None
            }
            .retryable()
        );
        assert!(
            !DownloadError::Integrity {
                path: PathBuf::new(),
                reason: String::new()
            }
            .retryable()
        );
    }
}
