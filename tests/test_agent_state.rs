use tokio_test::assert_ready;
use tokio_test::task::spawn;

#[test]
fn test_dummy_async() {
    // Basic test to verify tokio-test is available
    let mut task = spawn(async {
        1 + 1
    });

    assert_eq!(assert_ready!(task.poll()), 2);
}
