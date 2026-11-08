use launcher_core::commons_url;

#[test]
fn work_page_of_a_commons_id() {
    assert_eq!(
        commons_url("nc236081").map(String::from).as_deref(),
        Some("https://commons.nicovideo.jp/works/nc236081")
    );
}

#[test]
fn surrounding_whitespace_is_ignored() {
    assert_eq!(
        commons_url(" nc1\n").map(String::from).as_deref(),
        Some("https://commons.nicovideo.jp/works/nc1")
    );
}

#[test]
fn text_that_is_not_a_commons_id_has_no_page() {
    for id in ["", "nc", "236081", "NC1", "nc1/../x", "nc1?a=b", "nc1 2"] {
        assert_eq!(commons_url(id), None, "{id:?}");
    }
}
