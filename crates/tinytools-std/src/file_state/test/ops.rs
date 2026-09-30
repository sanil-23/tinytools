//! Tests for read/write tracking, staleness checks, and path locks.

use crate::file_state::types::WriteStamp;
use crate::file_state::{FileStateCoordinator, ReadStamp};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Mutex;

fn fresh_coordinator() -> Arc<FileStateCoordinator> {
    Arc::new(FileStateCoordinator::new())
}

#[test]
fn record_and_check_no_staleness() -> anyhow::Result<()> {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/a.txt");
    coord.reads.write().insert(
        ("agent-a".to_string(), path.clone()),
        ReadStamp {
            mtime: SystemTime::now(),
            timestamp: Instant::now(),
            partial: false,
        },
    );
    let partial = coord
        .reads
        .read()
        .get(&("agent-a".to_string(), path.clone()))
        .map(|rs| rs.partial);
    assert_eq!(partial, Some(false));
    assert!(coord.writes.read().get(&path).is_none());
    Ok(())
}

#[test]
fn detect_sibling_write_staleness() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/b.txt");
    let read_time = Instant::now();
    coord.reads.write().insert(
        ("agent-a".to_string(), path.clone()),
        ReadStamp {
            mtime: SystemTime::now(),
            timestamp: read_time,
            partial: false,
        },
    );
    std::thread::sleep(Duration::from_millis(5));
    coord.writes.write().insert(
        path.clone(),
        WriteStamp {
            writer: "agent-b".to_string(),
            timestamp: Instant::now(),
        },
    );
    let stale = coord.stale_reads_for_parent("agent-a");
    assert_eq!(stale, vec![path]);
}

#[test]
fn own_write_does_not_trigger_staleness() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/c.txt");
    let now = Instant::now();
    coord.reads.write().insert(
        ("agent-a".to_string(), path.clone()),
        ReadStamp {
            mtime: SystemTime::now(),
            timestamp: now,
            partial: false,
        },
    );
    std::thread::sleep(Duration::from_millis(5));
    coord.writes.write().insert(
        path.clone(),
        WriteStamp {
            writer: "agent-a".to_string(),
            timestamp: Instant::now(),
        },
    );
    let stale = coord.stale_reads_for_parent("agent-a");
    assert!(stale.is_empty());
}

#[test]
fn partial_read_detected() -> anyhow::Result<()> {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/d.txt");
    coord.reads.write().insert(
        ("agent-a".to_string(), path.clone()),
        ReadStamp {
            mtime: SystemTime::now(),
            timestamp: Instant::now(),
            partial: true,
        },
    );
    let partial = coord
        .reads
        .read()
        .get(&("agent-a".to_string(), path.clone()))
        .map(|rs| rs.partial);
    assert_eq!(partial, Some(true));
    Ok(())
}

#[test]
fn parent_stale_files_detects_child_writes() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/e.txt");
    let parent_read_time = Instant::now();
    coord.reads.write().insert(
        ("parent".to_string(), path.clone()),
        ReadStamp {
            mtime: SystemTime::now(),
            timestamp: parent_read_time,
            partial: false,
        },
    );
    std::thread::sleep(Duration::from_millis(5));
    coord.writes.write().insert(
        path.clone(),
        WriteStamp {
            writer: "child-1".to_string(),
            timestamp: Instant::now(),
        },
    );
    let stale = coord.stale_reads_for_parent("parent");
    assert_eq!(stale, vec![path]);
}

#[test]
fn paths_written_by_collects_correctly() {
    let coord = fresh_coordinator();
    let p1 = PathBuf::from("/tmp/test/f1.txt");
    let p2 = PathBuf::from("/tmp/test/f2.txt");
    coord.record_write("child-1", p1.clone());
    coord.record_write("child-2", p2);
    let result = coord.paths_written_by(&["child-1".to_string()]);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get("child-1"), Some(&vec![p1]));
}

#[tokio::test]
async fn path_lock_serialises_access() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/lock.txt");
    let mutex = {
        let mut locks = coord.path_locks.write();
        locks
            .entry(path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };

    let guard = mutex.lock().await;
    assert!(mutex.try_lock().is_err());
    drop(guard);
    assert!(mutex.try_lock().is_ok());
}

#[tokio::test]
async fn global_api_tracks_reads_writes_and_locks() -> anyhow::Result<()> {
    use crate::file_state::{
        acquire_path_lock, check_partial_read, check_stale_read, init_global, parent_stale_files,
        record_read, record_write, try_global,
    };

    init_global(false);
    init_global(true);
    assert!(try_global().is_some());

    let path = PathBuf::from("/tmp/test/global-flow.txt");
    record_read(
        "reader",
        path.clone(),
        SystemTime::now(),
        true,
        Instant::now(),
    );
    assert!(check_partial_read("reader", &path).is_some());
    assert!(check_stale_read("reader", &path).is_none());

    record_read(
        "reader",
        path.clone(),
        SystemTime::now(),
        false,
        Instant::now(),
    );
    assert!(check_partial_read("reader", &path).is_none());

    std::thread::sleep(Duration::from_millis(5));
    record_write("writer", path.clone());
    let msg = check_stale_read("reader", &path)
        .ok_or_else(|| anyhow::anyhow!("expected a stale read after the sibling write"))?;
    assert!(msg.contains("writer"));
    assert_eq!(
        parent_stale_files("reader", &["writer".to_string()]),
        vec![path.clone()]
    );
    assert!(check_stale_read("writer", &path).is_none());

    let guard = acquire_path_lock(&path).await;
    assert!(guard.is_some());
    Ok(())
}

#[test]
fn paths_written_by_keeps_a_path_after_another_agent_overwrites_it() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/history-shared.txt");
    coord.record_write("child-1", path.clone());
    coord.record_write("child-2", path.clone());

    let result = coord.paths_written_by(&["child-1".to_string(), "child-2".to_string()]);
    assert_eq!(result.get("child-1"), Some(&vec![path.clone()]));
    assert_eq!(result.get("child-2"), Some(&vec![path]));
}

#[test]
fn sibling_write_during_an_in_flight_read_is_reported_stale() {
    let coord = fresh_coordinator();
    let path = PathBuf::from("/tmp/test/in-flight.txt");
    let read_started = Instant::now();
    std::thread::sleep(Duration::from_millis(2));
    // The sibling write lands after the reader opened the file but before
    // the reader got round to recording the read.
    coord.record_write("sibling", path.clone());
    std::thread::sleep(Duration::from_millis(2));
    coord.record_read(
        "reader",
        path.clone(),
        SystemTime::now(),
        false,
        read_started,
    );

    assert_eq!(coord.stale_reads_for_parent("reader"), vec![path]);
}
