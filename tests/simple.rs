use first::{crash_point, test};

#[test]
fn simple_crash_test() {
    test()
        .run(|env| {
            // Write first file
            std::fs::write(env.path("file1.txt"), "first").unwrap();
            crash_point("after_first_write");

            // Write second file
            std::fs::write(env.path("file2.txt"), "second").unwrap();
            crash_point("after_second_write");
        })
        .verify(|env, info| {
            // file1 should always exist (crash happens after it's written)
            let data1 = std::fs::read_to_string(env.path("file1.txt")).unwrap();
            assert_eq!(data1, "first");

            // file2 only exists if we crashed after the second write
            if info.point_id == 2 {
                let data2 = std::fs::read_to_string(env.path("file2.txt")).unwrap();
                assert_eq!(data2, "second");
            }
        })
        .execute();
}
