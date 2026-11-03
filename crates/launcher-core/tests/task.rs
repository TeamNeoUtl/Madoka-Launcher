use launcher_core::{InstanceId, InstanceLocks, Progress, ProgressSink, TaskBoard, TaskState};

#[test]
fn board_records_each_state() {
    let board = TaskBoard::default();
    let first = board.add("first");
    let second = board.add("second");
    assert!(board.has_active());
    assert_eq!(board.snapshot()[0].state, TaskState::Queued);

    first.running(1, 2);
    assert_eq!(
        board.snapshot()[0].state,
        TaskState::Running {
            done: 1,
            total: 2,
            transfer: None
        }
    );

    first.finish(&Ok::<(), String>(()));
    second.finish(&Err::<(), &str>("boom"));

    let tasks = board.snapshot();
    assert_eq!(tasks[0].state, TaskState::Succeeded);
    assert_eq!(
        tasks[1].state,
        TaskState::Failed {
            reason: "boom".to_owned()
        }
    );
    assert_ne!(tasks[0].id, tasks[1].id);
    assert_eq!(tasks[0].title, "first");
    assert!(!board.has_active());
}

#[test]
fn transfer_is_recorded_only_while_running_and_reset_by_the_next_step() {
    let board = TaskBoard::default();
    let handle = board.add("download");
    let first = Progress {
        done: 5,
        total: Some(10),
    };

    handle.report(first);
    assert_eq!(board.snapshot()[0].state, TaskState::Queued);

    handle.running(0, 2);
    handle.report(first);
    assert_eq!(
        board.snapshot()[0].state,
        TaskState::Running {
            done: 0,
            total: 2,
            transfer: Some(first)
        }
    );

    handle.running(1, 2);
    assert_eq!(
        board.snapshot()[0].state,
        TaskState::Running {
            done: 1,
            total: 2,
            transfer: None
        }
    );
}

#[tokio::test]
async fn same_instance_is_serialized() {
    let locks = InstanceLocks::default();
    let id = InstanceId::generate();
    let guard = locks.lock(id).await;

    let waiter = tokio::spawn({
        let locks = locks.clone();
        async move {
            let _guard = locks.lock(id).await;
        }
    });
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert!(!waiter.is_finished());

    drop(guard);
    waiter.await.expect("waiter completes after release");
}

#[tokio::test]
async fn different_instances_do_not_block_each_other() {
    let locks = InstanceLocks::default();
    let held = locks.lock(InstanceId::generate()).await;

    let other = tokio::spawn({
        let locks = locks.clone();
        async move {
            let _guard = locks.lock(InstanceId::generate()).await;
        }
    });
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert!(other.is_finished());

    drop(held);
}

#[test]
fn sink_reports_to_the_task_of_its_handle() {
    let board = TaskBoard::default();
    let handle = board.add("update");
    let sink = handle.sink();
    handle.running(0, 1);
    sink.report(Progress {
        done: 5,
        total: Some(10),
    });
    assert_eq!(
        board.snapshot()[0].state,
        TaskState::Running {
            done: 0,
            total: 1,
            transfer: Some(Progress {
                done: 5,
                total: Some(10)
            })
        }
    );
}
