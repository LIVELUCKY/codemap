use super::*;

#[test]
fn a_far_longer_name_is_not_a_close_name() {
    assert!(covers("BRepoImpl", "BookRepositoryImpl", 0.3));
    assert!(covers("bookrout", "BookRoute", 0.3));
    assert!(
        !covers(
            "frontend",
            "test_search_filters_on_type_and_site_section",
            0.3
        ),
        "letters scattered over a long name"
    );
}
