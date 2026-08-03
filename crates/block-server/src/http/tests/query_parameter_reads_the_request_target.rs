use super::*;

fn head(path: &str) -> RequestHead {
    RequestHead {
        method: "GET".into(),
        path: path.into(),
        headers: Vec::new(),
    }
}

#[test]
fn query_parameter_reads_the_request_target() {
    let request = head("/?account=abc&workspace=def");
    assert_eq!(request.query_parameter("account"), Some("abc"));
    assert_eq!(request.query_parameter("workspace"), Some("def"));
    assert_eq!(request.query_parameter("missing"), None);

    // A parameter whose name is a suffix of another must not match it.
    assert_eq!(head("/?xaccount=abc").query_parameter("account"), None);
    // A target with no query string has no parameters.
    assert_eq!(head("/").query_parameter("account"), None);
    // A bare flag has no value to report.
    assert_eq!(head("/?account").query_parameter("account"), None);
}
