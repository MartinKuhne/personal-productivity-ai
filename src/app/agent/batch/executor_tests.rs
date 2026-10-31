//! Tests for [`BatchJobExecutor`] (`agent::batch::executor`).

use super::*;
use crate::bus::events::typed::BackgroundEventSender;
use std::sync::atomic::AtomicBool;
use tokio::sync::broadcast;

#[test]
fn test_execute_concurrent_empty() {
    let (tx, _rx) = broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let bus = std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver);
    let cancel_flag = Arc::new(AtomicBool::new(false));

    let executor = BatchJobExecutor::new(
        AppConfig::default(),
        bus,
        tx,
        "test prompt".to_string(),
        cancel_flag,
        Arc::new(crate::utils::clock::SystemClock),
    );

    let result = executor.execute_concurrent(vec![], 4);
    assert_eq!(result.total_jobs, 0);
    assert_eq!(result.completed, 0);
    assert_eq!(result.failed, 0);
    assert_eq!(result.cancelled, 0);
}

#[test]
fn test_execute_concurrent_cancellation() {
    let (tx, _rx) = broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let bus = std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver);
    let cancel_flag = Arc::new(AtomicBool::new(true));

    let executor = BatchJobExecutor::new(
        AppConfig::default(),
        bus,
        tx,
        "test prompt".to_string(),
        cancel_flag,
        Arc::new(crate::utils::clock::SystemClock),
    );

    let jobs = vec![BatchJob {
        id: 0,
        target_path: PathBuf::from("/tmp/test1.md"),
        active_file: Some(PathBuf::from("/tmp/test1.md")),
        active_dir: None,
        prompt_text: "test".to_string(),
        status: BatchJobStatus::Pending,
        start_time: None,
        end_time: None,
        error: None,
    }];

    let result = executor.execute_concurrent(jobs, 4);
    assert_eq!(result.total_jobs, 1);
    assert_eq!(result.cancelled, 1);
}
/// Build `count` pending jobs with distinct ids and paths.
fn pending_jobs(count: usize) -> Vec<BatchJob> {
    (0..count)
        .map(|id| {
            let path = PathBuf::from(format!("/tmp/batch_{id}.md"));
            BatchJob {
                id,
                target_path: path.clone(),
                active_file: Some(path),
                active_dir: None,
                prompt_text: "test".to_string(),
                status: BatchJobStatus::Pending,
                start_time: None,
                end_time: None,
                error: None,
            }
        })
        .collect()
}

/// Build an executor wired to an in-memory background channel and a
/// pre-set cancellation flag, so no LLM call is ever attempted.
fn cancelled_executor() -> BatchJobExecutor {
    let (tx, _rx) = broadcast::channel(16);
    let bus = std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver);
    BatchJobExecutor::new(
        AppConfig::default(),
        bus,
        BackgroundEventSender::new(tx),
        "test prompt".to_string(),
        Arc::new(AtomicBool::new(true)),
        Arc::new(crate::utils::clock::SystemClock),
    )
}

/// A pre-cancelled batch must mark *every* job cancelled and never dispatch
/// one to the agent: the counters have to partition the total exactly, for
/// any number of jobs.
#[test]
fn test_execute_concurrent_cancels_every_job_without_dispatching() {
    let executor = cancelled_executor();

    for count in [1usize, 3, 7] {
        let result = executor.execute_concurrent(pending_jobs(count), 4);

        assert_eq!(
            result.total_jobs, count,
            "{count} jobs submitted: the total must echo the submission"
        );
        assert_eq!(
            result.cancelled, count,
            "{count} jobs submitted with cancellation already requested: every \
             one must be counted cancelled, got {result:?}"
        );
        assert_eq!(
            result.completed, 0,
            "{count} cancelled jobs: nothing can have completed, got {result:?}"
        );
        assert_eq!(
            result.failed, 0,
            "{count} cancelled jobs: cancellation is not a failure, got {result:?}"
        );
        assert_eq!(
            result.cancelled + result.completed + result.failed,
            result.total_jobs,
            "{count} jobs submitted: every job must land in exactly one bucket"
        );
    }
}

/// The reported duration is measured from the executor's injected clock and
/// must stay finite for a batch that did no work.
#[test]
fn test_execute_concurrent_reports_a_finite_duration_for_a_cancelled_batch() {
    let executor = cancelled_executor();
    let result = executor.execute_concurrent(pending_jobs(2), 2);

    assert!(
        result.duration.as_secs() < 60,
        "a batch that dispatched nothing must not report a long run, got {:?}",
        result.duration
    );
}

#[test]
fn a_zero_concurrency_request_is_clamped_to_serial_execution() {
    // A zero-permit semaphore can never be acquired: without the clamp a caller
    // passing 0 would hang the batch forever ([FMD-064]).
    assert_eq!(
        BatchJobExecutor::effective_concurrency(0),
        1,
        "a request for zero parallelism must still permit one job at a time"
    );
    assert_eq!(
        BatchJobExecutor::effective_concurrency(1),
        1,
        "one stays one"
    );
    assert_eq!(
        BatchJobExecutor::effective_concurrency(8),
        8,
        "a sane request is passed through untouched"
    );
    assert_eq!(
        BatchJobExecutor::effective_concurrency(u8::MAX),
        usize::from(u8::MAX),
        "the widest request must not overflow or be truncated"
    );
}
